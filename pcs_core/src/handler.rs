use http::{Request, Response, header};
use serde::Deserialize;

use crate::{
    file::{self, CompleteUploadParams, CreateFileTokenParams},
    game::{self, CreateGameSaveParams, UpdateGameSaveParams},
    types::{backend::PCSBackend, error::PCSError, event::Event},
    user::{self, AuthData, UpdateUserParams},
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

    pub async fn on(&self, event: Event) {
        self.backend.emit_event(event).await;
    }

    pub async fn handler(&self, req: Request<Vec<u8>>) -> Response<Vec<u8>> {
        match self.dispatch(&req).await {
            Ok(resp) => resp,
            Err(err) => err.into(),
        }
    }

    async fn dispatch(&self, req: &Request<Vec<u8>>) -> Result<Response<Vec<u8>>, PCSError> {
        let method = req.method().as_str();
        let path = req.uri().path();
        let body = req.body().as_slice();
        let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match (method, segs.as_slice()) {
            // User routes
            ("POST", ["1.1", "users"]) => {
                let rb: RegisterBody = serde_json::from_slice(body).map_bad_err()?;
                created(&user::handle_register(&self.backend, rb.auth_data.taptap).await?)
            }
            ("GET", ["1.1", "users", "me"]) => {
                let st = PhiCloudServer::<B>::session_token(req)?;
                ok(&user::handle_get_current(&self.backend, st).await?)
            }
            ("PUT", ["1.1", "users", obj_id]) | ("PUT", ["1.1", "classes", "_User", obj_id]) => {
                let params: UpdateUserParams = serde_json::from_slice(body).map_bad_err()?;
                user::handle_update(&self.backend, obj_id, params).await?;
                no_content()
            }
            ("PUT", ["1.1", "users", obj_id, "refreshSessionToken"]) => {
                let st = PhiCloudServer::<B>::session_token(req)?;
                ok(&user::handle_refresh_token(&self.backend, obj_id, st).await?)
            }
            ("DELETE", ["1.1", "users", obj_id]) => {
                let st = PhiCloudServer::<B>::session_token(req)?;
                user::handle_delete(&self.backend, obj_id, st).await?;
                no_content()
            }

            // File routes
            ("POST", ["1.1", "fileTokens"]) => {
                let params: CreateFileTokenParams = serde_json::from_slice(body).map_bad_err()?;
                let server_url = self.get_server_url(&req)?;
                created(&file::handle_create_token(&self.backend, params, &server_url).await?)
            }
            ("GET", ["1.1", "files", obj_id]) => {
                let data = file::handle_download(&self.backend, obj_id).await?;
                Ok(Response::builder()
                    .status(200)
                    .header("Content-Type", "application/octet-stream")
                    .header("Cache-Control", "public, max-age=31536000, immutable")
                    .body(data)
                    .map_bad_err()?)
            }
            ("DELETE", ["1.1", "files", obj_id]) => {
                file::handle_delete(&self.backend, obj_id).await?;
                no_content()
            }
            ("POST", ["1.1", "fileCallback"]) => ok(&file::handle_callback(&self.backend).await?),

            // Upload routes
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
                let params: CompleteUploadParams = serde_json::from_slice(body).map_bad_err()?;
                ok(
                    &file::handle_complete_upload(
                        &self.backend,
                        token_key,
                        upload_id,
                        params.parts,
                    )
                    .await?,
                )
            }

            // Game save routes
            ("GET", ["1.1", "classes", "_GameSave"]) => {
                let st = PhiCloudServer::<B>::session_token(req)?;
                let server_url = self.get_server_url(&req)?;
                ok(&game::handle_list(&self.backend, st, &server_url).await?)
            }
            ("POST", ["1.1", "classes", "_GameSave"]) => {
                let st = PhiCloudServer::<B>::session_token(req)?;
                let params: CreateGameSaveParams = serde_json::from_slice(body).map_bad_err()?;
                created(&game::handle_create(&self.backend, st, params).await?)
            }
            ("PUT", ["1.1", "classes", "_GameSave", obj_id]) => {
                let st = PhiCloudServer::<B>::session_token(req)?;
                let params: UpdateGameSaveParams = serde_json::from_slice(body).map_bad_err()?;
                game::handle_update(&self.backend, obj_id, st, params).await?;
                no_content()
            }

            _ => Err(PCSError::not_found("route not found")),
        }
    }

    fn session_token<'a>(req: &'a Request<Vec<u8>>) -> Result<&'a str, PCSError> {
        req.headers()
            .get("X-LC-Session")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| PCSError::unauthorized("missing session token"))
    }

    fn get_server_url(&self,req: &Request<Vec<u8>>) -> Result<String, PCSError> {
        let host = req.headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| PCSError::bad_request("missing host"))?;
        
        Ok(self.backend.scheme() + "://" + host)
    }
}
