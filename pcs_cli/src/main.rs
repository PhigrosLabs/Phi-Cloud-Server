mod backend;
mod config;
mod file_bucket;
mod kv;
mod tokios;
mod utils;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use backend::CliBackend;
use bytes::Bytes;
use clap::Parser;
use config::Config;
use file_bucket::FileFileBucket;
use http::{Request, Response};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use kv::RedbKVStorage;
use pcs_core::handler::PhiCloudServer;
use tokio::net::TcpListener;

use crate::tokios::TokioIo;
use crate::utils::UnsafeSendFuture;

type AppState = Arc<PhiCloudServer<CliBackend>>;

type RespBody = Full<Bytes>;

async fn body_to_vec<B>(body: B) -> Result<Vec<u8>, B::Error>
where
    B: http_body::Body<Data = Bytes>,
{
    let collected = body.collect().await?;
    let bytes = collected.to_bytes();

    Ok(bytes.to_vec())
}

async fn handle_req(
    server: AppState,
    req: Request<Incoming>,
) -> Result<Response<RespBody>, hyper::Error> {
    let (parts, body) = req.into_parts();

    let body = body_to_vec(body).await?;

    let new_req = Request::from_parts(parts, body);

    let resp = server.handler(new_req).await;

    let (parts, body) = resp.into_parts();

    let body_bytes = match body {
        Some(mut stream) => {
            let mut data = Vec::new();

            use futures::StreamExt;

            while let Some(chunk) = stream.next().await {
                data.extend_from_slice(&chunk);
            }

            Bytes::from(data)
        }
        None => Bytes::new(),
    };

    let resp = Response::from_parts(parts, Full::new(body_bytes));

    Ok(resp)
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

    let fb = FileFileBucket::new(data_dir.join("file"));

    let backend = CliBackend {
        kv,
        fb,

        webhook: if config.webhook_url.is_empty() {
            None
        } else {
            Some(config.webhook_url.clone())
        },

        scheme: config.scheme.clone(),

        http_client: reqwest::Client::new(),
    };

    let server = Arc::new(PhiCloudServer::new(backend));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let listener = TcpListener::bind(addr).await?;

    println!("Listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);
        let service_server = server.clone();

        let service = service_fn(move |req| handle_req(service_server.clone(), req));

        tokio::task::spawn(
            // SAFETY:
            // 该 future 实际满足 Send，
            // 当前 stable rustc 因生命周期/HRTB 推导限制无法证明。
            //
            // nightly + -Zhigher-ranked-assumptions 可通过正常类型检查
            // https://github.com/rust-lang/rust/issues/100013
            //
            // 已确认 future 捕获状态均为 Send，
            // 不包含线程亲和的 !Send 状态。
            UnsafeSendFuture(async move {
                if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                    println!("Error serving connection: {:?}", err);
                }
            }),
        );
    }
}
