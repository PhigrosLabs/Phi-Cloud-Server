use alloc::vec::Vec;
use http::{Request, Response};
use send_future::SendFuture;
use serde::Deserialize;

use crate::{
    file, game,
    types::{PcsBody, backend::PCSBackend, error::PCSError},
    user::{self, AuthData},
    utils::{MapPCSError, created, no_content, ok, pcs_body_from_stream},
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

    pub async fn handler(&self, req: Request<Vec<u8>>) -> Response<PcsBody> {
        // 跨crate有推断问题 https://github.com/rust-lang/rust/issues/100013 此issue解决后可删除.send()和send-future = "0.1"
        match self.dispatch(req).send().await {
            Ok(resp) => resp,
            Err(err) => err.into(),
        }
    }

    async fn dispatch(&self, req: Request<Vec<u8>>) -> Result<Response<PcsBody>, PCSError> {
        let (parts, body) = req.into_parts();

        let method = parts.method.as_str();
        let path = parts.uri.path();
        let headers = parts.headers;

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
                ok(&user::handle_update(&self.backend, obj_id, params).await?)
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
                let st = Self::session_token(&headers)?;
                let server_url = self.backend.server_url();
                created(&file::handle_create_token(&self.backend, st, params, &server_url).await?)
            }

            ("GET", ["1.1", "files", obj_id]) => {
                let stream = file::handle_download(&self.backend, obj_id).await?;
                Ok(Response::new(pcs_body_from_stream(stream)))
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
                let server_url = self.backend.server_url();

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

                ok(&game::handle_update(&self.backend, obj_id, st, params).await?)
            }

            // =========================
            // Extension routes
            // =========================
            #[cfg(feature = "extension_save")]
            ("GET", ["extension", "save", session_token]) => {
                use crate::extensions::save::handler::handle_save_extension;
                let query = parts.uri.query();
                ok(&handle_save_extension(&self.backend, session_token, query).await?)
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
}
