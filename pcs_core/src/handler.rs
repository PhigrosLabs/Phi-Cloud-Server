use alloc::vec::Vec;
use send_future::SendFuture;
use serde::Deserialize;

use crate::{
    file, game,
    types::{
        error::{ErrorCode, PCSError},
        *,
    },
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
            // https://developer.taptap.cn/docs/v3/sdk/authentication/rest/
            // 仅支持TapTap登录,非全部实现
            // =========================

            // /1.1/users POST 用户注册 用户连接
            ("POST", ["1.1", "users"]) | ("POST", ["1.1", "classes", "_User"]) => {
                let rb: RegisterBody =
                    serde_json::from_slice(body).map_pcs_bad(ErrorCode::JSON_DESERIALIZE)?;
                created(&user::handle_register(backend, rb.auth_data.taptap).await?)
            }

            // /1.1/users/me GET 根据 sessionToken 获取用户信息
            ("GET", ["1.1", "users", "me"]) => {
                ok(&user::handle_get_current(backend, Self::st(st)?).await?)
            }

            // /1.1/users/<objectId> PUT 更新用户 用户连接 验证Email
            // 仅支持更新用户名
            ("PUT", ["1.1", "users", obj_id]) | ("PUT", ["1.1", "classes", "_User", obj_id]) => {
                let params =
                    serde_json::from_slice(body).map_pcs_bad(ErrorCode::JSON_DESERIALIZE)?;
                ok(&user::handle_update(backend, obj_id, params).await?)
            }

            // /1.1/users/<objectId>/refreshSessionToken PUT 刷新 sessionToken
            ("PUT", ["1.1", "users", obj_id, "refreshSessionToken"]) => {
                ok(&user::handle_refresh_token(backend, obj_id, Self::st(st)?).await?)
            }

            // /1.1/users/<objectId> DELETE 删除用户
            ("DELETE", ["1.1", "users", obj_id])
            | ("DELETE", ["1.1", "classes", "_User", obj_id]) => {
                user::handle_delete(backend, obj_id, Self::st(st)?).await?;
                no_content()
            }

            // =========================
            // File routes
            // https://developer.taptap.cn/docs/v3/sdk/storage/guide/rest/#%E6%96%87%E4%BB%B6
            // 非全部实现
            // =========================

            // 私有接口
            ("POST", ["1.1", "fileTokens"]) => {
                let params =
                    serde_json::from_slice(body).map_pcs_bad(ErrorCode::JSON_DESERIALIZE)?;
                created(
                    &file::handle_create_token(backend, Self::st(st)?, params, server_url).await?,
                )
            }

            // 私有接口
            ("GET", ["1.1", "files", obj_id]) => {
                let stream = file::handle_download(backend, obj_id).await?;
                Ok(Response {
                    status_code: 200,
                    body: Some(Body::ByteStream(stream)),
                    content_type: Some(OCTET_STREAM_CONTENT_TYPE.into()),
                })
            }

            // https://developer.taptap.cn/docs/v3/sdk/storage/guide/rest/#%E5%88%A0%E9%99%A4%E6%96%87%E4%BB%B6
            ("DELETE", ["1.1", "files", obj_id]) => {
                file::handle_delete(backend, obj_id).await?;
                no_content()
            }

            // 私有接口
            ("POST", ["1.1", "fileCallback"]) => ok(&file::handle_callback(backend).await?),

            // =========================
            // Upload routes
            // 私有接口
            // =========================
            ("POST", ["buckets", bucket, "objects", token_key, "uploads"]) => {
                created(&file::handle_start_upload(backend, bucket, token_key).await?)
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
                let pn: u16 = part_num.parse().map_err(|_| {
                    PCSError::bad_request(ErrorCode::INVALID_PART_NUMBER, "invalid part number")
                })?;

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
                let params =
                    serde_json::from_slice(body).map_pcs_bad(ErrorCode::JSON_DESERIALIZE)?;
                ok(&file::handle_complete_upload(backend, token_key, upload_id, params).await?)
            }

            // =========================
            // Game save routes
            // https://developer.taptap.cn/docs/v3/sdk/gamesaves/guide/#%E6%8E%A5%E5%8F%A3%E5%88%97%E8%A1%A8
            // =========================

            // 查询存档 GET /gamesaves 根据查询条件查询存档
            ("GET", ["1.1", "classes", "_GameSave"]) | ("GET", ["1.1", "gamesaves"]) => {
                ok(&game::handle_list(backend, Self::st(st)?, server_url).await?)
            }

            // 添加存档 POST /gamesaves 增加新存档
            ("POST", ["1.1", "classes", "_GameSave"]) | ("POST", ["1.1", "gamesaves"]) => {
                let params =
                    serde_json::from_slice(body).map_pcs_bad(ErrorCode::JSON_DESERIALIZE)?;

                created(&game::handle_create(backend, Self::st(st)?, params).await?)
            }

            // 更新存档 PUT /gamesaves/:id 根据 id 更新存档
            ("PUT", ["1.1", "classes", "_GameSave", obj_id])
            | ("PUT", ["1.1", "gamesaves", obj_id]) => {
                let params =
                    serde_json::from_slice(body).map_pcs_bad(ErrorCode::JSON_DESERIALIZE)?;

                ok(&game::handle_update(backend, obj_id, Self::st(st)?, params).await?)
            }

            // 获取存档 GET /gamesaves/:id 根据 id 来获取存档记录
            ("GET", ["1.1", "classes", "_GameSave", obj_id])
            | ("GET", ["1.1", "gamesaves", obj_id]) => {
                ok(&game::handle_get(backend, obj_id, Self::st(st)?, server_url).await?)
            }

            // 删除存档 DELETE /gamesaves/:id 根据 id 删除文档 (taptap把存档打错成文档了)
            ("DELETE", ["1.1", "classes", "_GameSave", obj_id])
            | ("DELETE", ["1.1", "gamesaves", obj_id]) => {
                game::handle_delete(backend, obj_id, Self::st(st)?).await?;
                no_content()
            }

            // =========================
            // Extension routes
            // =========================
            #[cfg(feature = "extension_query_b30")]
            ("GET", ["extension", "b30", session_token]) => {
                use crate::extensions::b30::handler::handle_b30_extension_get;
                let svg = handle_b30_extension_get(backend, session_token).await?;
                Ok(Response {
                    status_code: 200,
                    content_type: Some(SVG_CONTENT_TYPE.into()),
                    body: Some(Body::Bytes(svg.into_bytes())),
                })
            }

            #[cfg(feature = "extension_save")]
            ("GET", ["extension", "save", session_token]) => {
                use crate::extensions::save::handler::handle_save_extension_get;
                ok(&handle_save_extension_get(backend, session_token).await?)
            }

            #[cfg(feature = "extension_save")]
            ("PUT", ["extension", "save", session_token]) => {
                use crate::extensions::save::handler::handle_save_extension_put;
                handle_save_extension_put(backend, session_token, body).await?;
                no_content()
            }

            _ => Err(PCSError::not_found(
                ErrorCode::ROUTE_NOT_FOUND,
                "route not found",
            )),
        }
    }

    fn st(session_token: Option<&str>) -> Result<&str, PCSError> {
        session_token.ok_or(PCSError::unauthorized(
            ErrorCode::MISSING_SESSION_TOKEN,
            "missing session token",
        ))
    }
}
