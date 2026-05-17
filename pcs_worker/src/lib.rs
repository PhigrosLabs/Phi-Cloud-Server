mod backend;
mod kv;
mod utils;

use backend::{FileMode, WorkerBackend};
use kv::WorkerKVStorage;
use pcs_core::handler::PhiCloudServer;
use worker::*;

#[event(fetch)]
async fn fetch(req: HttpRequest, env: Env, _ctx: Context) -> Result<Response> {
    let webhook_url = env
        .var("WEBHOOK_URL")
        .ok()
        .map(|s| s.to_string())
        .and_then(|s| if s.is_empty() { None } else { Some(s) });

    let file_mode = match env
        .var("FILE_MODE")
        .map(|s| s.to_string())
        .unwrap_or_default()
        .as_str()
    {
        "R2" => FileMode::R2,
        "KV" => FileMode::Kv,
        _ => panic!("不支持"),
    };

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

    // File storage: R2 or KV
    let r2 = if matches!(file_mode, FileMode::R2) {
        let bucket_name = env
            .var("R2_BUCKET")
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "PHI_BUCKET".into());
        Some(env.bucket(&bucket_name)?)
    } else {
        None
    };

    let file_kv = if matches!(file_mode, FileMode::Kv) {
        let file_kv_namespace = env
            .var("FILE_KV_NAMESPACE")
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "FILE_KV".into());
        Some(env.kv(&file_kv_namespace)?)
    } else {
        None
    };

    let backend = WorkerBackend {
        db_kv,
        file_kv,
        r2,
        file_mode,
        webhook: webhook_url,
        scheme,
    };

    let server = PhiCloudServer::new(backend);

    let resp = server.handler(req).await;

    worker::Response::try_from(resp)
}
