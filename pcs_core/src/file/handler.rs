use alloc::vec::Vec;

use crate::{
    file::{types::*, utils::*},
    types::{
        backend::PCSBackend,
        error::{ErrorCode, PCSError},
        file_bucket::{FileBucket, MultipartUpload, UploadedPart},
    },
    user,
};

use crate::utils::*;

pub async fn handle_create_token<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    params: CreateFileTokenBody,
    server_url: &str,
) -> Result<CreateFileTokenResponse, PCSError> {
    user::get_session_by_token(backend, session_token).await?;
    let ft = FileTokenInfo::new(backend, params.meta_data.into()).await?;
    Ok(ft.to_response(server_url))
}

pub async fn handle_delete<B: PCSBackend>(backend: &B, object_id: &str) -> Result<(), PCSError> {
    delete_file(backend, object_id).await?;
    Ok(())
}

pub async fn handle_download<B: PCSBackend>(
    backend: &B,
    object_id: &str,
) -> Result<<B::FB as FileBucket>::Stream, PCSError> {
    let (_, stream) = get_file(backend, object_id)
        .await?
        .ok_or(PCSError::not_found(ErrorCode::FB_GET, "file not found"))?;
    Ok(stream)
}

pub async fn handle_callback<B: PCSBackend>(
    _backend: &B,
) -> Result<FileCallbackResponse, PCSError> {
    Ok(FileCallbackResponse {
        result: true,
        token: FILE_UPTOKEN.into(),
    })
}

pub async fn handle_start_upload<B: PCSBackend>(
    _backend: &B,
    bucket: &str,
    _token_key: &str,
) -> Result<StartUploadResponse, PCSError> {
    Ok(StartUploadResponse {
        upload_id: bucket.into(),
    })
}

pub async fn handle_upload_part<B: PCSBackend>(
    backend: &B,
    token_key: &str,
    upload_id: &str,
    part_number: u16,
    data: &[u8],
) -> Result<UploadPartResponse, PCSError> {
    let key = decode_base64_key(token_key)?;

    let fb = backend.fb();
    let mut upload = fb
        .get_multipart_upload(&key, upload_id)
        .await
        .map_pcs_error(ErrorCode::FB_GET_MULTIPART_UPLOAD)?;
    let part = upload
        .upload_part(part_number, data)
        .await
        .map_pcs_error(ErrorCode::FB_UPLOAD_PART)?;

    Ok(UploadPartResponse { etag: part.etag })
}

pub async fn handle_complete_upload<B: PCSBackend>(
    backend: &B,
    token_key: &str,
    upload_id: &str,
    params: CompleteUploadBody,
) -> Result<CompleteUploadResponse, PCSError> {
    let key = decode_base64_key(token_key)?;
    let fb = backend.fb();
    let mut upload = fb
        .get_multipart_upload(&key, upload_id)
        .await
        .map_pcs_error(ErrorCode::FB_GET_MULTIPART_UPLOAD)?;

    let upload_parts: Vec<UploadedPart> = params
        .parts
        .into_iter()
        .map(|p| UploadedPart::new(p.part_number, p.etag))
        .collect();

    upload
        .complete(upload_parts)
        .await
        .map_pcs_error(ErrorCode::FB_MULTIPART_COMPLETE)?;

    Ok(CompleteUploadResponse { key })
}
