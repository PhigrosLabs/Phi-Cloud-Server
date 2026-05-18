mod backend;
mod kv;
mod utils;

use std::sync::Arc;

use backend::WorkerBackend;
use kv::WorkerKVStorage;
use pcs_core::handler::PhiCloudServer;
use worker::*;

use crate::utils::stream_to_vec;

#[event(fetch)]
async fn fetch(req: HttpRequest, env: Env, _ctx: Context) -> Result<Response> {
    let webhook_url = env
        .var("WEBHOOK_URL")
        .ok()
        .map(|s| s.to_string())
        .and_then(|s| if s.is_empty() { None } else { Some(s) });

    let scheme = env
        .var("SCHEME")
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "https".into());

    let db_kv_namespace = env
        .var("DB_KV_NAMESPACE")
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "DB".into());

    let db_kv = WorkerKVStorage {
        kv: env.kv(&db_kv_namespace)?,
        table_prefix: String::new(),
    };

    let bucket_name = env
        .var("R2_BUCKET")
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "PHI_BUCKET".into());
    let r2 = env.bucket(&bucket_name)?;

    let backend = WorkerBackend {
        db_kv,
        r2,
        webhook: webhook_url,
        scheme,
    };

    let server = Arc::new(PhiCloudServer::new(backend));

    let (parts, body) = req.into_parts();
    let new_req = http::Request::from_parts(parts, stream_to_vec(body).await?);
    let resp = server.handler(new_req).await;

    utils::build_response(resp).await
}
