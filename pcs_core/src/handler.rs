use alloc::vec::Vec;
use send_future::SendFuture;
use serde::Deserialize;

use crate::{
    file, game,
    types::*,
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

pub struct PhiCloudServer;

impl PhiCloudServer {
    pub async fn handler<B: PCSBackend>(
        backend: &B,
        req: Request<'_>,
    ) -> Response<<B::FB as FileBucket>::Stream> {
        // 跨crate有推断问题 https://github.com/rust-lang/rust/issues/100013 此issue解决后可删除.send()和send-future = "0.1"
        match Self::dispatch(backend, req).send().await {
            Ok(resp) => resp,
            Err(err) => err.into(),
        }
    }

    async fn dispatch<B: PCSBackend>(
        backend: &B,
        req: Request<'_>,
    ) -> Result<Response<<B::FB as FileBucket>::Stream>, PCSError> {
        let method = req.method;
        let path = req.path;
        let body = req.body;
        let st = req.session_token;
        let server_url = req.server_url;

        let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match (method, segs.as_slice()) {
            // =========================
            // User routes
            // =========================
            ("POST", ["1.1", "users"]) => {
                let rb: RegisterBody = serde_json::from_slice(&body).map_bad_err()?;
                created(&user::handle_register(backend, rb.auth_data.taptap).await?)
            }

            ("GET", ["1.1", "users", "me"]) => {
                ok(&user::handle_get_current(backend, Self::st(st)?).await?)
            }

            ("PUT", ["1.1", "users", obj_id]) | ("PUT", ["1.1", "classes", "_User", obj_id]) => {
                let params = serde_json::from_slice(&body).map_bad_err()?;
                ok(&user::handle_update(backend, obj_id, params).await?)
            }

            ("PUT", ["1.1", "users", obj_id, "refreshSessionToken"]) => {
                ok(&user::handle_refresh_token(backend, obj_id, Self::st(st)?).await?)
            }

            ("DELETE", ["1.1", "users", obj_id]) => {
                user::handle_delete(backend, obj_id, Self::st(st)?).await?;
                no_content()
            }

            // =========================
            // File routes
            // =========================
            ("POST", ["1.1", "fileTokens"]) => {
                let params = serde_json::from_slice(&body).map_bad_err()?;
                created(
                    &file::handle_create_token(backend, Self::st(st)?, params, server_url).await?,
                )
            }

            ("GET", ["1.1", "files", obj_id]) => {
                let stream = file::handle_download(backend, obj_id).await?;
                Ok(Response {
                    status_code: 200,
                    body: Some(Body::ByteStream(stream)),
                    content_type: Some(OCTET_STREAM_CONTENT_TYPE.into()),
                })
            }

            ("DELETE", ["1.1", "files", obj_id]) => {
                file::handle_delete(backend, obj_id).await?;
                no_content()
            }

            ("POST", ["1.1", "fileCallback"]) => ok(&file::handle_callback(backend).await?),

            // =========================
            // Upload routes
            // =========================
            ("POST", ["buckets", _bucket, "objects", token_key, "uploads"]) => {
                created(&file::handle_start_upload(backend, token_key).await?)
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

                ok(&file::handle_upload_part(backend, token_key, upload_id, pn, body).await?)
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
                ok(&file::handle_complete_upload(backend, token_key, upload_id, params).await?)
            }

            // =========================
            // Game save routes
            // =========================
            ("GET", ["1.1", "classes", "_GameSave"]) => {
                ok(&game::handle_list(backend, Self::st(st)?, server_url).await?)
            }

            ("POST", ["1.1", "classes", "_GameSave"]) => {
                let params = serde_json::from_slice(&body).map_bad_err()?;

                created(&game::handle_create(backend, Self::st(st)?, params).await?)
            }

            ("PUT", ["1.1", "classes", "_GameSave", obj_id]) => {
                let params = serde_json::from_slice(&body).map_bad_err()?;

                ok(&game::handle_update(backend, obj_id, Self::st(st)?, params).await?)
            }

            // =========================
            // Extension routes
            // =========================
            #[cfg(feature = "extension_save")]
            ("GET", ["extension", "save", session_token]) => {
                use crate::extensions::save::handler::handle_save_extension_get;
                ok(&handle_save_extension_get(backend, session_token).await?)
            }

            #[cfg(feature = "extension_save")]
            ("PUT", ["extension", "save", session_token]) => {
                use crate::extensions::save::handler::handle_save_extension_put;
                handle_save_extension_put(backend, session_token, &body).await?;
                no_content()
            }

            _ => Err(PCSError::not_found("route not found")),
        }
    }

    fn st(session_token: Option<&str>) -> Result<&str, PCSError> {
        session_token.ok_or_else(|| PCSError::unauthorized("missing session token"))
    }
}
