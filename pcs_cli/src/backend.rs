use pcs_core::{
    types::{
        backend::{PCSBackend, PhiInfoResponse, UserCheckResult},
        error::{ErrorCode, PCSError},
        event::Event,
    },
    user::AuthData,
};

use crate::file_bucket::LocalFileBucket;
use crate::kv::RedbKVStorage;

pub struct CliBackend {
    pub kv: RedbKVStorage,
    pub fb: LocalFileBucket,
    pub webhook: Option<String>,
    pub server_url: String,
    pub phi_info_url: String,
    pub http_client: reqwest::Client,
}

impl PCSBackend for CliBackend {
    type FB = LocalFileBucket;
    type KV = RedbKVStorage;
    type Error = PCSError;

    fn fb(&self) -> &Self::FB {
        &self.fb
    }

    fn kv(&self) -> &Self::KV {
        &self.kv
    }

    async fn user_check(&self, auth: &AuthData) -> Result<UserCheckResult, PCSError> {
        let Some(ref url) = self.webhook else {
            return Ok(UserCheckResult::default());
        };

        let webhook_url = format!("{}/pcs/user_check", url);

        let resp = self
            .http_client
            .post(&webhook_url)
            .json(auth)
            .send()
            .await
            .map_err(|e| PCSError::internal_error(ErrorCode::other(90048), e.to_string()))?;

        if resp.status() != 200 {
            return Err(PCSError::internal_error(
                ErrorCode::other(90051),
                format!("webhook user_check returned status {}", resp.status()),
            ));
        }

        resp.json()
            .await
            .map_err(|e| PCSError::internal_error(ErrorCode::other(90059), e.to_string()))
    }

    async fn emit_event(&self, event: Event) {
        let Some(ref url) = self.webhook else {
            return;
        };

        let webhook_url = format!("{}/pcs/event", url);
        let _ = self
            .http_client
            .post(&webhook_url)
            .json(&event)
            .send()
            .await;
    }

    fn random_id(&self) -> String {
        random_id()
    }

    fn utc_now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    async fn call_phi_info_router(&self, path: &str) -> Result<PhiInfoResponse, Self::Error> {
        if self.phi_info_url.is_empty() {
            return Err(PCSError::internal_error(
                ErrorCode::other(90086),
                "phi_info_url is not configured",
            ));
        }

        if let Some(base) = self.phi_info_url.strip_prefix("file://") {
            let file_path = std::path::PathBuf::from(base).join(path.trim_start_matches('/'));
            let data = std::fs::read(&file_path).map_err(|e| {
                PCSError::internal_error(
                    ErrorCode::other(90092),
                    format!("failed to read {}: {}", file_path.display(), e),
                )
            })?;
            let mime = guess_mime_from_path(path);
            Ok(PhiInfoResponse {
                code: 200,
                mime,
                data,
            })
        } else if self.phi_info_url.starts_with("http://")
            || self.phi_info_url.starts_with("https://")
        {
            let url = join_http_url(&self.phi_info_url, path);
            let resp = self.http_client.get(&url).send().await.map_err(|e| {
                PCSError::internal_error(
                    ErrorCode::other(90106),
                    format!("phi_info request failed: {}", e),
                )
            })?;

            let code = resp.status().as_u16();
            let mime = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            let data = resp
                .bytes()
                .await
                .map_err(|e| {
                    PCSError::internal_error(
                        ErrorCode::other(90120),
                        format!("phi_info read response failed: {}", e),
                    )
                })?
                .to_vec();

            Ok(PhiInfoResponse { code, mime, data })
        } else {
            Err(PCSError::internal_error(
                ErrorCode::other(90126),
                format!("unsupported phi_info_url scheme: {}", self.phi_info_url),
            ))
        }
    }
}

pub(crate) fn random_id() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    const BASE: u128 = 36;

    let mut buf = [0u8; 18];
    getrandom::getrandom(&mut buf).expect("getrandom error");

    let mut n = 0u128;
    for &b in &buf {
        n = (n << 8) | b as u128;
    }

    let mut out = [0u8; 25];
    for i in (0..25).rev() {
        let idx = (n % BASE) as usize;
        out[i] = CHARSET[idx];
        n /= BASE;
    }

    String::from_utf8_lossy(&out).to_string()
}

fn guess_mime_from_path(path: &str) -> String {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "json" => "application/json".into(),
        "png" => "image/png".into(),
        "jpg" | "jpeg" => "image/jpeg".into(),
        "svg" => "image/svg+xml".into(),
        "webp" => "image/webp".into(),
        "gif" => "image/gif".into(),
        "mp3" => "audio/mpeg".into(),
        "ogg" => "audio/ogg".into(),
        "wav" => "audio/wav".into(),
        "txt" => "text/plain; charset=utf-8".into(),
        "html" => "text/html; charset=utf-8".into(),
        _ => "application/octet-stream".into(),
    }
}

fn join_http_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{}/{}", base, path)
}
