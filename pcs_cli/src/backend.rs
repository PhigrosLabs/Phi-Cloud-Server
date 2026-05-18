use async_trait::async_trait;
use pcs_core::{
    types::{
        backend::{PCSBackend, UserCheckResult},
        error::PCSError,
        event::Event,
    },
    user::AuthData,
};

use crate::file_bucket::FileFileBucket;
use crate::kv::RedbKVStorage;

pub struct CliBackend {
    pub kv: RedbKVStorage,
    pub fb: FileFileBucket,
    pub webhook: Option<String>,
    pub scheme: String,
    pub http_client: reqwest::Client,
}

#[async_trait]
impl PCSBackend for CliBackend {
    type FB = FileFileBucket;
    type KV = RedbKVStorage;

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
            .map_err(|e| PCSError::internal_error(e.to_string()))?;

        if resp.status() != 200 {
            return Err(PCSError::internal_error(format!(
                "webhook user_check returned status {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| PCSError::internal_error(e.to_string()))
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

    fn scheme(&self) -> String {
        self.scheme.clone()
    }

    fn random_id(&self) -> String {
        random_id()
    }

    fn get_utc_now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
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
