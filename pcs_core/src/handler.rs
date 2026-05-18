use core::error::Error;

use alloc::{format, string::String, vec::Vec};
use bytes::Bytes;
use futures::{Stream, TryStreamExt};
use http::{Request, Response, header};
use serde::Deserialize;

use crate::{
    file, game,
    types::{PcsBody, backend::PCSBackend, error::PCSError},
    user::{self, AuthData},
    utils::{MapPCSError, created, no_content, ok, pcs_body_from_stream},
};

async fn stream_to_vec<S, E>(stream: S) -> Result<Vec<u8>, E>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Error,
{
    let chunks: Vec<Bytes> = stream.try_collect().await?;
    let total_len: usize = chunks.iter().map(|c| c.len()).sum();
    let mut result = Vec::with_capacity(total_len);
    for chunk in chunks {
        result.extend_from_slice(&chunk);
    }

    Ok(result)
}

#[derive(Deserialize)]
struct RegisterBody {
    #[serde(rename = "authData")]
    auth_data: AuthDataWrapper,
}

#[derive(Deserialize)]
struct AuthDataWrapper {
    taptap: AuthData,
}

pub struct PhiCloudServer<B: PCSBackend> {
    backend: B,
}

impl<B: PCSBackend> PhiCloudServer<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub async fn handler(
        &self,
        req: Request<impl Stream<Item = Result<Bytes, impl Error>> + Unpin>,
    ) -> Response<PcsBody> {
        match self.dispatch(req).await {
            Ok(resp) => resp,
            Err(err) => err.into(),
        }
    }

    async fn dispatch(
        &self,
        req: Request<impl Stream<Item = Result<Bytes, impl Error>> + Unpin>,
    ) -> Result<Response<PcsBody>, PCSError> {
        let (parts, body) = req.into_parts();

        let method = parts.method.as_str();
        let path = parts.uri.path();
        let headers = parts.headers;
        let body = stream_to_vec(body).await.map_bad_err()?;

        let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match (method, segs.as_slice()) {
            // =========================
            // User routes
            // =========================
            ("POST", ["1.1", "users"]) => {
                let rb: RegisterBody = serde_json::from_slice(&body).map_bad_err()?;
                created(&user::handle_register(&self.backend, rb.auth_data.taptap).await?)
            }

            ("GET", ["1.1", "users", "me"]) => {
                let st = Self::session_token(&headers)?;
                ok(&user::handle_get_current(&self.backend, st).await?)
            }

            ("PUT", ["1.1", "users", obj_id]) | ("PUT", ["1.1", "classes", "_User", obj_id]) => {
                let params = serde_json::from_slice(&body).map_bad_err()?;
                user::handle_update(&self.backend, obj_id, params).await?;
                no_content()
            }

            ("PUT", ["1.1", "users", obj_id, "refreshSessionToken"]) => {
                let st = Self::session_token(&headers)?;
                ok(&user::handle_refresh_token(&self.backend, obj_id, st).await?)
            }

            ("DELETE", ["1.1", "users", obj_id]) => {
                let st = Self::session_token(&headers)?;
                user::handle_delete(&self.backend, obj_id, st).await?;
                no_content()
            }

            // =========================
            // File routes
            // =========================
            ("POST", ["1.1", "fileTokens"]) => {
                let params = serde_json::from_slice(&body).map_bad_err()?;
                let server_url = self.get_server_url(&headers)?;
                created(&file::handle_create_token(&self.backend, params, &server_url).await?)
            }

            ("GET", ["1.1", "files", obj_id]) => {
                let stream = file::handle_download(&self.backend, obj_id).await?;

                Response::builder()
                    .status(200)
                    .header("Content-Type", "application/octet-stream")
                    .header("Cache-Control", "public, max-age=31536000, immutable")
                    .body(pcs_body_from_stream(stream))
                    .map_bad_err()
            }

            ("DELETE", ["1.1", "files", obj_id]) => {
                file::handle_delete(&self.backend, obj_id).await?;
                no_content()
            }

            ("POST", ["1.1", "fileCallback"]) => ok(&file::handle_callback(&self.backend).await?),

            // =========================
            // Upload routes
            // =========================
            ("POST", ["buckets", _bucket, "objects", token_key, "uploads"]) => {
                created(&file::handle_start_upload(&self.backend, token_key).await?)
            }

            (
                "PUT",
                [
                    "buckets",
                    _bucket,
                    "objects",
                    token_key,
                    "uploads",
                    upload_id,
                    part_num,
                ],
            ) => {
                let pn: u32 = part_num
                    .parse()
                    .map_err(|_| PCSError::bad_request("invalid part number"))?;

                ok(&file::handle_upload_part(
                    &self.backend,
                    token_key,
                    upload_id,
                    pn,
                    body.to_vec(),
                )
                .await?)
            }

            (
                "POST",
                [
                    "buckets",
                    _bucket,
                    "objects",
                    token_key,
                    "uploads",
                    upload_id,
                ],
            ) => {
                let params = serde_json::from_slice(&body).map_bad_err()?;
                ok(
                    &file::handle_complete_upload(&self.backend, token_key, upload_id, params)
                        .await?,
                )
            }

            // =========================
            // Game save routes
            // =========================
            ("GET", ["1.1", "classes", "_GameSave"]) => {
                let st = Self::session_token(&headers)?;
                let server_url = self.get_server_url(&headers)?;

                ok(&game::handle_list(&self.backend, st, &server_url).await?)
            }

            ("POST", ["1.1", "classes", "_GameSave"]) => {
                let st = Self::session_token(&headers)?;
                let params = serde_json::from_slice(&body).map_bad_err()?;

                created(&game::handle_create(&self.backend, st, params).await?)
            }

            ("PUT", ["1.1", "classes", "_GameSave", obj_id]) => {
                let st = Self::session_token(&headers)?;
                let params = serde_json::from_slice(&body).map_bad_err()?;

                game::handle_update(&self.backend, obj_id, st, params).await?;
                no_content()
            }

            _ => Err(PCSError::not_found("route not found")),
        }
    }

    fn session_token(headers: &http::HeaderMap) -> Result<&str, PCSError> {
        headers
            .get("X-LC-Session")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| PCSError::unauthorized("missing session token"))
    }

    fn get_server_url(&self, headers: &http::HeaderMap) -> Result<String, PCSError> {
        let host = headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| PCSError::bad_request("missing host"))?;

        Ok(format!("{}://{}", self.backend.scheme(), host))
    }
}
