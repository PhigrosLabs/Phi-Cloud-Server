use bytes::Bytes;
use http::{Request, Response, header};
use http_body::Body;
use http_body_util::BodyExt;
use serde::Deserialize;

use crate::{
    PcsBody, file, game, pcs_body_from_stream,
    types::{backend::PCSBackend, error::PCSError},
    user::{self, AuthData},
    utils::{MapPCSError, created, no_content, ok},
};

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

    pub async fn handler<BY>(&self, req: Request<BY>) -> Response<PcsBody>
    where
        BY: Body<Data = Bytes> + Send,
        BY::Error: std::error::Error,
    {
        match self.dispatch(req).await {
            Ok(resp) => resp,
            Err(err) => err.into(),
        }
    }

    async fn dispatch<BY>(&self, req: Request<BY>) -> Result<Response<PcsBody>, PCSError>
    where
        BY: Body<Data = Bytes> + Send,
        BY::Error: std::error::Error,
    {
        let (parts, body) = req.into_parts();

        let method = parts.method.as_str();
        let path = parts.uri.path();
        let headers = parts.headers;

        let body = body.collect().await.map_bad_err()?.to_bytes();

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
