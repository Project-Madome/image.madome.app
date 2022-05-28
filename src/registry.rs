use sai::{component_registry, Component};

use crate::{app::HttpServer, config::Config};

component_registry!(RootRegistry, [Config, HttpServer]);
