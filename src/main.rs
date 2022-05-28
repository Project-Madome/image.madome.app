use log::Level;
use madome_image::{release, RootRegistry};
use sai::System;
use tokio::signal::{self, unix::SignalKind};

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let log_level = if release() { Level::Info } else { Level::Debug };

    simple_logger::init_with_level(log_level).unwrap();

    let mut system = System::<RootRegistry>::new();

    system.start().await;

    let mut sigterm = signal::unix::signal(SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = sigterm.recv() => {},
        _ = async { signal::ctrl_c().await.expect("failed to listen for ctrl_c event") } => {}
    };

    system.stop().await;

    log::info!("gracefully shutdown the app");
}
