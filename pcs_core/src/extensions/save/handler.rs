use alloc::string::String;
use alloc::vec::Vec;
use futures::stream::StreamExt;
use serde::Serialize;

use phi_save_codec::{GameKey, GameProgress, GameRecord, Settings, User};

use super::save_provider::SaveProvider;
use crate::{
    file,
    game::model::GameSave,
    types::{
        backend::PCSBackend,
        error::PCSError,
        file_bucket::FileBucket,
        kv::{KVStorage, KVTable},
    },
    user,
    utils::MapPCSError,
};

#[derive(Debug, Default, Serialize)]
pub struct SaveExtensionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_key: Option<GameKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_record: Option<GameRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_progress: Option<GameProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Settings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn parse_file_params(query: Option<&str>) -> Vec<String> {
    let mut files = Vec::new();
    if let Some(q) = query {
        for pair in q.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                if key == "file" && !value.is_empty() {
                    files.push(value.into());
                }
            }
        }
    }
    files
}

pub async fn handle_save_extension<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    query: Option<&str>,
) -> Result<SaveExtensionResponse, PCSError> {
    let files = parse_file_params(query);

    let session = user::get_session_by_token(backend, session_token).await?;

    let kv = backend.kv();
    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;

    let gs_ids: Vec<String> = games_by_user
        .get(&session.object_id)
        .await
        .map_db_err()?
        .unwrap_or_default();

    if gs_ids.is_empty() {
        return Err(PCSError::not_found("no game saves found"));
    }

    let last_gs_id = gs_ids.last().unwrap();
    let gs: GameSave = game_saves
        .get(last_gs_id)
        .await
        .map_db_err()?
        .ok_or_else(PCSError::db_not_found)?;

    let ft = file::get_file_token(backend, &gs.game_file_object_id).await?;
    let fb = backend.fb();
    let mut stream = fb.get(ft.key).await.map_internal_err()?;

    let mut data = Vec::new();
    while let Some(chunk) = stream.next().await {
        data.extend_from_slice(&chunk);
    }

    let provider = SaveProvider::new(&data)
        .map_err(|e| PCSError::bad_request(alloc::format!("invalid save data: {:?}", e)))?;

    let mut response = SaveExtensionResponse::default();
    for file in &files {
        match file.as_str() {
            "game_key" => {
                response.game_key = Some(
                    provider
                        .get_game_key()
                        .map_err(|e| PCSError::internal_error(e))?,
                );
            }
            "game_record" => {
                response.game_record = Some(
                    provider
                        .get_game_record()
                        .map_err(|e| PCSError::internal_error(e))?,
                );
            }
            "game_progress" => {
                response.game_progress = Some(
                    provider
                        .get_game_progress()
                        .map_err(|e| PCSError::internal_error(e))?,
                );
            }
            "settings" => {
                response.settings = Some(
                    provider
                        .get_settings()
                        .map_err(|e| PCSError::internal_error(e))?,
                );
            }
            "user" => {
                response.user = Some(
                    provider
                        .get_user()
                        .map_err(|e| PCSError::internal_error(e))?,
                );
            }
            "name" => {
                response.name = Some(session.nickname.clone());
            }
            _ => {}
        }
    }

    Ok(response)
}
