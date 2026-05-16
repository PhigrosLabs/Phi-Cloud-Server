mod backend;
mod kv;
mod utils;

use backend::WorkerBackend;
use futures_util::TryStreamExt;
use kv::WorkerKVStorage;
use pcs_core::handler::PhiCloudServer;
use worker::*;

#[event(fetch)]
async fn fetch(req: HttpRequest, env: Env, _ctx: Context) -> Result<HttpResponse> {
    let webhook_url = env
        .var("WEBHOOK_URL")
        .ok()
        .map(|s| s.to_string())
        .and_then(|s| if s.is_empty() { None } else { Some(s) });

    let file_mode = env
        .var("FILE_MODE")
        .map(|s| s.to_string())
        .unwrap_or_default();

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
    let r2 = if file_mode == "R2" {
        let bucket_name = env
            .var("R2_BUCKET")
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "PHI_BUCKET".into());
        Some(env.bucket(&bucket_name)?)
    } else {
        None
    };

    let file_kv = if file_mode != "R2" && !file_mode.is_empty() {
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
        file_mode: file_mode.clone(),
        webhook: webhook_url,
        scheme,
    };

    let server = PhiCloudServer::new(backend);

    let (parts, body) = req.into_parts();
    let body_bytes = Box::pin(body)
        .try_fold(Vec::new(), |mut acc, chunk| async move {
            acc.extend_from_slice(&chunk);
            Ok(acc)
        })
        .await
        .map_err(|e| worker::Error::RustError(e.to_string()))?;

    let http_req = http::Request::from_parts(parts, body_bytes);

    let resp = server.handler(http_req).await;

    let (parts, body_vec) = resp.into_parts();
    let mut builder = ResponseBuilder::new().with_status(parts.status.as_u16());

    for (name, value) in &parts.headers {
        if let Ok(v) = value.to_str() {
            builder = builder.with_header(name.as_str(), v)?;
        }
    }

    let worker_resp = builder.body(ResponseBody::Body(body_vec));
    let http_resp: HttpResponse = worker_resp.try_into()?;
    Ok(http_resp)
}
