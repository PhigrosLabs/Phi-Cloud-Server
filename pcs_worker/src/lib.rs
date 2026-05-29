mod backend;
mod kv;
mod utils;

use backend::WorkerBackend;
use kv::WorkerKVStorage;
use pcs_core::handler::PhiCloudServer;
use worker::*;

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let webhook_url = env
        .var("WEBHOOK_URL")
        .ok()
        .map(|s| s.to_string())
        .and_then(|s| if s.is_empty() { None } else { Some(s) });

    let server_url = env
        .var("SERVER_URL")
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "http://127.0.0.1:8787".into());

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

    let user_count_limit: u32 = env
        .var("USER_COUNT")
        .ok()
        .and_then(|s| s.to_string().parse().ok())
        .unwrap_or(0);

    let backend = WorkerBackend {
        db_kv,
        r2,
        webhook: webhook_url,
        user_count_limit,
    };

    let body = req.bytes().await?;
    let method = req.method().to_string();
    let path = req.path().to_string();
    let st = req.headers().get("X-LC-Session").ok().and_then(|h| h);

    let pcs_req = pcs_core::types::Request {
        method: &method,
        path: &path,
        body,
        session_token: st.as_deref(),
        server_url: &server_url,
    };

    let resp = PhiCloudServer::handler(&backend, pcs_req).await;

    utils::build_response(resp).await
}
