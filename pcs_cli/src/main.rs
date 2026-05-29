mod backend;
mod config;
mod file_bucket;
mod kv;
mod tokios;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use backend::CliBackend;
use bytes::Bytes;
use clap::Parser;
use config::Config;
use file_bucket::LocalFileBucket;
use futures::TryStreamExt;
use http::{Request, Response};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use kv::RedbKVStorage;
use pcs_core::handler::PhiCloudServer;
use pcs_core::types::ByteStream;
use tokio::net::TcpListener;

use crate::tokios::TokioIo;

type AppState = Arc<CliBackend>;

async fn stream_to_bytes<S: ByteStream>(stream: S) -> Vec<u8> {
    let chunks: Result<Vec<Vec<u8>>, _> = stream.try_collect().await;
    let chunks = chunks.unwrap_or_default();
    let total_len: usize = chunks.iter().map(|c| c.len()).sum();
    let mut result = Vec::with_capacity(total_len);
    for chunk in chunks {
        result.extend_from_slice(&chunk);
    }

    result
}

async fn handle_req(
    state: AppState,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let (parts, body) = req.into_parts();

    let method = parts.method.as_str();
    let path = parts.uri.path();
    let session_token = parts
        .headers
        .get("X-LC-Session")
        .and_then(|v| v.to_str().ok());

    let body = body.collect().await?.to_bytes().to_vec();

    let pcs_req = pcs_core::types::Request {
        method,
        path,
        body,
        session_token,
        server_url: &state.server_url,
    };

    let resp = PhiCloudServer::handler(state.as_ref(), pcs_req).await;

    let mut builder = Response::builder().status(resp.status_code);

    if let Some(ct) = &resp.content_type {
        builder = builder.header("Content-Type", ct.clone());
    }

    let response = match resp.body {
        Some(pcs_core::types::Body::Bytes(bytes)) => builder.body(Full::new(bytes.into())).unwrap(),
        Some(pcs_core::types::Body::ByteStream(stream)) => {
            let data = stream_to_bytes(stream).await;
            builder.body(Full::new(Bytes::from(data))).unwrap()
        }
        None => builder.body(Full::new(Bytes::new())).unwrap(),
    };

    Ok(response)
}

#[derive(Parser)]
#[command(name = "pcs_cli", about = "Phi Cloud Server CLI")]
struct Cli {
    /// Config file path
    #[arg(short = 'c', long = "config", default_value = "./config.json")]
    config: PathBuf,

    /// Listen port
    #[arg(short = 'p', long = "port", default_value = "3000")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    let config = Config::load(cli.config.to_str().expect("invalid config path"))
        .expect("failed to load config");

    let port = cli.port;

    let data_dir = &config.data_dir;

    std::fs::create_dir_all(data_dir.join("file")).expect("failed to create data dirs");

    let kv_path = data_dir.join("kv.db");

    let kv = RedbKVStorage::new(kv_path.to_str().unwrap()).unwrap();

    let fb = LocalFileBucket::new(data_dir.join("file"));

    let backend = CliBackend {
        kv,
        fb,

        webhook: if config.webhook_url.is_empty() {
            None
        } else {
            Some(config.webhook_url.clone())
        },

        server_url: config.server_url.clone(),

        http_client: reqwest::Client::new(),
    };

    let backend = Arc::new(backend);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let listener = TcpListener::bind(addr).await?;

    println!("Listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);
        let service_server = backend.clone();

        let service = service_fn(move |req| handle_req(service_server.clone(), req));

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
