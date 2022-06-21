use std::{
    convert::Infallible,
    ffi::OsString,
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use either::Either;
use http_util::{Multipart, ReadChunks, SetResponse};
use hyper::{
    header,
    service::{make_service_fn, service_fn},
    Body, HeaderMap, Method, Request, Response, Server, StatusCode,
};
use hyper_staticfile::Static;
use inspect::{Inspect, InspectOk};
use refract_core::{ImageKind, FLAG_NO_LOSSLESS};
use sai::{Component, ComponentLifecycle, Injected};
use serde::Serialize;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    sync::oneshot,
};
use util::elapse;

use crate::config::Config;

async fn auth(headers: &HeaderMap) -> crate::Result<()> {
    let cookie = headers
        .get(header::COOKIE)
        .ok_or(crate::Error::Unauthenticated)?;

    let resp = reqwest::Client::new()
        .get("https://test.api.madome.app/auth/token")
        .header(header::COOKIE, cookie)
        .send()
        .await?;

    match resp.status() {
        StatusCode::OK => Ok(()),

        StatusCode::UNAUTHORIZED => Err(crate::Error::Unauthenticated),

        code => Err(crate::Error::UnknownStatusCode(
            code,
            resp.text().await.unwrap_or_default(),
        )),
    }
}

#[derive(Component)]
#[lifecycle]
pub struct HttpServer {
    #[injected]
    config: Injected<Config>,

    stop_sender: Option<oneshot::Sender<()>>,
    stopped_receiver: Option<oneshot::Receiver<()>>,
}

async fn handler(
    method: Method,
    mut uri_path: String,
    mut request: Request<Body>,
    r#static: Static,
) -> crate::Result<Response<Body>> {
    auth(request.headers()).await?;

    // remove first slash character
    let uri_path = {
        while let Some('/') = uri_path.chars().next() {
            uri_path.remove(0);
        }

        // compatible from old api
        if uri_path.starts_with("v1") {
            uri_path.replace("v1", "")
        } else {
            uri_path
        }
    };

    let resp = match (method, uri_path) {
        (Method::GET, uri_path) if uri_path.ends_with("image_list") => {
            let p = r#static.root.join(uri_path);

            let mut dir = match p.parent() {
                Some(p) => fs::read_dir(p).await?,
                None => todo!(),
            };

            #[derive(Serialize)]
            struct Item(#[serde(with = "either::serde_untagged")] pub Either<String, OsString>);

            let mut xs: Vec<Item> = Vec::new();

            while let Some(x) = dir.next_entry().await? {
                /* let x = x.path().file_name().map(|x| x.to_string_lossy());
                let x = x.into_string(); */

                let x = x.file_name().into_string();

                match x {
                    Ok(x) => xs.push(Item(Either::Left(x))),
                    Err(x) => xs.push(Item(Either::Right(x))),
                }
            }

            let body = serde_json::to_vec(&xs).unwrap();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::CONTENT_LENGTH, body.len())
                .body(body.into())
                .unwrap()
        }

        (Method::GET, _uri_path) => {
            let mut resp = r#static.clone().serve(request).await?;

            let content_type = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|x| x.to_str().ok())
                .unwrap_or_default();

            // avif to webp
            if content_type.contains("image/avif") {
                let body = resp.body_mut();

                let buf = body.read_chunks().await.unwrap();

                /*

                for &e in encoders {
                    Share::sync(tx, rx, Ok(Share::Encoder(e)));
                    if let Ok(mut guide) = EncodeIter::new(&src, e, flags) {
                        let mut count: u8 = 0;
                        while let Some(can) = guide.advance().and_then(|out| Candidate::try_from(out).ok()) {
                            count += 1;
                            let res = Share::sync(tx, rx, Ok(Share::Candidate(can.with_count(count))));
                            match res {
                                ShareFeedback::Keep => { guide.keep(); },
                                ShareFeedback::Discard => { guide.discard(); },
                                ShareFeedback::Abort => { break; },
                                _ => {},
                            }
                        }

                        // Save the best, if any!
                        Share::sync(tx, rx, guide.take().map(|x| Share::Best(path.to_path_buf(), x)));
                    }
                }

                            */

                let input = refract_core::Input::try_from(buf.as_slice())?;

                let mut it =
                    refract_core::EncodeIter::new(&input, ImageKind::Webp, FLAG_NO_LOSSLESS)?;

                // consume
                // while let Some(_) = it.advance() {}
                /* while let Some(output) = it.advance() {
                    let quality = output.quality().quality();

                    if let QualityValue::Int(x) = quality {
                        log::debug!("{x}");
                    }
                } */

                let _r = it.advance().unwrap();

                // let r = it.candidate().unwrap();

                it.keep();

                let output = it.take().unwrap();
                // let output = it.advance().unwrap();

                log::debug!("{:#?}", output.quality());

                let buf = output.iter().copied().collect::<Vec<_>>();
                let size = buf.len();

                *body = buf.into();

                {
                    let headers = resp.headers_mut();

                    headers.remove(header::CONTENT_TYPE);
                    headers.remove(header::CONTENT_LENGTH);
                }

                resp.set_header(header::CONTENT_TYPE, "image/webp").unwrap();
                resp.set_header(header::CONTENT_LENGTH, size).unwrap();

                resp
            } else {
                resp
            }
        }

        (Method::PUT, uri_path) => {
            let file_path = r#static.root.join(uri_path);

            let mut multipart = Multipart::new(&mut request).await?;

            let (_headers, content) = multipart.next().unwrap();

            match file_path.parent() {
                // do not access the parent of root
                Some(parent) if !r#static.root.starts_with(parent) => {
                    log::debug!("root = {:?}", r#static.root);
                    log::debug!("file_path = {file_path:?}");
                    log::debug!("dir = {parent:?}");

                    fs::create_dir_all(parent).await?;

                    let mut file = File::create(&file_path).await?;

                    file.write_all(&content).await?;

                    Response::builder()
                        .status(StatusCode::NO_CONTENT)
                        .body(Body::empty())
                        .unwrap()
                }
                _ => Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body("Permission denied".into())
                    .unwrap(),
            }
        }

        // NOT FOUND
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not found".into())
            .unwrap(),
    };

    Ok(resp)
}

async fn service(request: Request<Body>, r#static: Static) -> Result<Response<Body>, Infallible> {
    let req_method = request.method().to_owned();
    let req_path = request.uri().path().to_string();

    log::info!("--> {} {}", req_method, req_path);

    let start = SystemTime::now();

    let response = elapse!(
        "execute",
        handler(req_method.clone(), req_path.clone(), request, r#static).await
    );

    let end = start
        .elapsed()
        .as_ref()
        .map(Duration::as_micros)
        .unwrap_or(0);

    match response {
        Ok(response) => Ok(response),
        Err(err) => {
            let code = match err {
                crate::Error::Unauthenticated => StatusCode::UNAUTHORIZED,
                crate::Error::UnknownStatusCode(code, _) => code,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };

            let err = err.inspect(|e| log::error!("{}", e)).to_string().into();

            let resp = Response::builder().status(code).body(err).unwrap();

            Ok(resp)
        }
    }
    .inspect_ok(|res| {
        log::info!(
            "<-- {} {} {} {}ms",
            req_method,
            req_path,
            res.status().as_u16(),
            end as f64 / 1000.0
        )
    })

    // log::error!("{}", err);
}

#[async_trait::async_trait]
impl ComponentLifecycle for HttpServer {
    async fn start(&mut self) {
        let (stop_tx, stop_rx) = oneshot::channel();
        let (stopped_tx, stopped_rx) = oneshot::channel();

        self.stop_sender.replace(stop_tx);
        self.stopped_receiver.replace(stopped_rx);

        let r#static = Static::new(self.config.base_path());

        let port = self.config.port();
        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        tokio::spawn(async move {
            let svc = |r#static: Static| async move {
                Ok::<_, Infallible>(service_fn(move |request| {
                    service(request, r#static.clone())
                }))
            };

            let server = Server::bind(&addr).serve(make_service_fn(move |_| svc(r#static.clone())));

            let server = Server::with_graceful_shutdown(server, async {
                stop_rx.await.unwrap();
            });

            log::info!("started http server: 0.0.0.0:{}", port);

            if let Err(err) = server.await {
                log::error!("{:?}", err);
            }

            stopped_tx.send(()).unwrap();
        });
    }

    async fn stop(&mut self) {
        let stop_tx = self.stop_sender.take().unwrap();

        stop_tx.send(()).unwrap();

        let stopped_rx = self.stopped_receiver.take().unwrap();

        stopped_rx.await.unwrap();
    }
}
