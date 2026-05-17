use bytes::Bytes;
use futures::Stream;

use crate::{
    file::{model::*, types::*, utils::*},
    types::{
        backend::PCSBackend,
        error::PCSError,
        file_bucket::{FileBucket, MultipartUpload, UploadedPart},
        kv::{KVStorage, KVTable},
    },
};

use crate::utils::*;

pub async fn handle_create_token<B: PCSBackend>(
    backend: &B,
    params: CreateFileTokenParams,
    server_url: &str,
) -> Result<FileTokenResponse, PCSError> {
    let ft = FileToken::new(params.meta_data, params.name, params.acl, backend);

    save_file_token(backend, &ft).await?;
    Ok(ft.to_response(server_url))
}

pub async fn handle_delete<B: PCSBackend>(backend: &B, object_id: &str) -> Result<(), PCSError> {
    let ft = get_file_token(backend, object_id).await?;

    let fb = backend.file_bucket().await;
    fb.delete(&ft.key).await.map_internal_err()?;

    let kv = backend.kv().await;
    let file_tokens = kv.open_table("file_tokens").await.map_db_err()?;
    file_tokens.delete(&ft.object_id).await.map_db_err()?;
    let ft_by_key = kv.open_table("file_tokens_by_key").await.map_db_err()?;
    ft_by_key.delete(&ft.key).await.map_db_err()?;

    Ok(())
}

pub async fn handle_download<B: PCSBackend>(
    backend: &B,
    object_id: &str,
) -> Result<impl Stream<Item = Bytes> + Send + Sync + 'static, PCSError> {
    let ft = get_file_token(backend, object_id).await?;
    let fb = backend.file_bucket().await;
    fb.get_data(ft.key).await.map_internal_err()
}

pub async fn handle_callback<B: PCSBackend>(
    _backend: &B,
) -> Result<FileCallbackResponse, PCSError> {
    Ok(FileCallbackResponse { result: true })
}

pub async fn handle_start_upload<B: PCSBackend>(
    backend: &B,
    token_key: &str,
) -> Result<StartUploadResponse, PCSError> {
    let key = decode_base64_key(token_key)?;
    let ft = get_file_token(backend, &key).await?;

    let fb = backend.file_bucket().await;
    let upload_id = fb
        .create_multipart_upload(&ft.key)
        .await
        .map_internal_err()?;

    Ok(StartUploadResponse { upload_id })
}

pub async fn handle_upload_part<B: PCSBackend>(
    backend: &B,
    token_key: &str,
    upload_id: &str,
    part_number: u32,
    data: Vec<u8>,
) -> Result<UploadPartResponse, PCSError> {
    let key = decode_base64_key(token_key)?;
    let ft = get_file_token(backend, &key).await?;

    let fb = backend.file_bucket().await;
    let mut upload = fb
        .get_multipart_upload(&ft.key, upload_id)
        .await
        .map_internal_err()?;
    let part = upload
        .upload_part(part_number, data)
        .await
        .map_internal_err()?;

    Ok(UploadPartResponse { etag: part.etag })
}

pub async fn handle_complete_upload<B: PCSBackend>(
    backend: &B,
    token_key: &str,
    upload_id: &str,
    params: CompleteUploadParams,
) -> Result<CompleteUploadResponse, PCSError> {
    let key = decode_base64_key(token_key)?;
    let ft = get_file_token(backend, &key).await?;

    let fb = backend.file_bucket().await;
    let mut upload = fb
        .get_multipart_upload(&ft.key, upload_id)
        .await
        .map_internal_err()?;

    let upload_parts: Vec<UploadedPart> = params
        .parts
        .into_iter()
        .map(|p| UploadedPart::new(p.part_number, p.etag))
        .collect();

    upload.complete(upload_parts).await.map_internal_err()?;

    Ok(CompleteUploadResponse {
        upload_id: upload_id.into(),
    })
}
