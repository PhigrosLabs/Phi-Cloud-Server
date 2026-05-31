use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use askama::Template;
use chrono::{DateTime, FixedOffset, Utc};

use super::template::*;
use super::types::SongInfo;
use super::utils::*;
use crate::extensions::save::save_provider::SaveProvider;
use crate::game::model::GameSave;
use crate::types::{
    PCSBackend, PCSError,
    file_bucket::FileBucket,
    kv::{KVStorage, KVTable},
};
use crate::user;
use crate::utils::{MapPCSError, stream_to_bytes};

pub async fn handle_b30_extension_get<B: PCSBackend>(
    backend: &B,
    session_token: &str,
) -> Result<String, PCSError> {
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

    let gs: GameSave = game_saves
        .get(&gs_ids[0])
        .await
        .map_db_err()?
        .ok_or_else(PCSError::db_not_found)?;
    let fb = backend.fb();
    let stream = fb.get(&gs.game_file_object_id).await.map_internal_err()?;
    let data = stream_to_bytes(stream).await.map_internal_err()?;
    let provider = SaveProvider::parse(&data)
        .map_err(|e| PCSError::bad_request(format!("invalid save data: {:?}", e)))?;

    let game_record = provider
        .get_game_record()
        .map_err(|e| PCSError::internal_error(e.to_string()))?;
    let user_info = provider
        .get_user()
        .map_err(|e| PCSError::internal_error(e.to_string()))?;
    let game_progress = provider
        .get_game_progress()
        .map_err(|e| PCSError::internal_error(e.to_string()))?;
    let settings = provider
        .get_settings()
        .map_err(|e| PCSError::internal_error(e.to_string()))?;

    let fetcher = PhiInfoFetcher::new(backend).await?;
    let songs = fetcher.get_songs().await?;
    let song_map: BTreeMap<String, SongInfo> =
        songs.into_iter().map(|s| (s.id.clone(), s)).collect();

    let (all_30, total_rks) = compute_b30(&song_map, &game_record.songs);

    let owned_cards = build_b30_cards(&fetcher, &song_map, &all_30).await?;
    let cards = into_card_data(&owned_cards);

    let utc_time: DateTime<Utc> = gs.updated_at;
    let offset_eight_hours = FixedOffset::east_opt(8 * 3600).unwrap();
    let date_str = utc_time.with_timezone(&offset_eight_hours).to_rfc3339();
    let info = build_personal_info(
        &fetcher,
        &user_info.background.0,
        game_progress.challenge_mode_rank,
        &game_progress.money,
        &settings.device_name.0,
        &date_str,
        total_rks,
    )
    .await?;

    let template = B30Template {
        bg_link: &info.bg_uri,
        icon_link: &info.icon_uri,
        challenge_link: &info.challenge_uri,
        font_link: &info.font_uri,
        player_id: &session.nickname,
        rks: &info.rks,
        challenge_mode_rank: info.challenge_mode_rank,
        data: &info.data,
        device: &info.device,
        date: &info.date,
        cards: &cards,
    };

    template
        .render()
        .map_err(|e| PCSError::internal_error(format!("template render failed: {}", e)))
}
