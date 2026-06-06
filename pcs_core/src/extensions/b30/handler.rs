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
use crate::game::model::{GameSave, GameSaveIdsByUser};
use crate::types::{
    PCSBackend, PCSError, error::ErrorCode, file_bucket::FileBucket, kv::KVStorage,
};
use crate::user;
use crate::utils::{MapPCSError, stream_to_bytes};

pub async fn handle_b30_extension_get<B: PCSBackend>(
    backend: &B,
    session_token: &str,
) -> Result<String, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;
    let kv = backend.kv();
    let GameSaveIdsByUser(gs_ids) = kv
        .get::<GameSaveIdsByUser>(&session.object_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .unwrap_or(GameSaveIdsByUser(Vec::new()));
    if gs_ids.is_empty() {
        return Err(PCSError::not_found(
            ErrorCode::B30_NO_GAME_SAVES_FOUND,
            "no game saves found",
        ));
    }

    let gs: GameSave = kv
        .get::<GameSave>(&gs_ids[0])
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .ok_or_else(PCSError::db_not_found)?;
    let fb = backend.fb();
    let stream = fb
        .get(&gs.game_file_object_id)
        .await
        .map_pcs_error(ErrorCode::FB_GET)?;
    let data = stream_to_bytes(stream)
        .await
        .map_pcs_error(ErrorCode::FB_GET)?;
    let provider = SaveProvider::parse(&data).map_err(|e| {
        PCSError::bad_request(
            ErrorCode::B30_INVALID_SAVE_DATA,
            format!("invalid save data: {:?}", e),
        )
    })?;

    let game_record = provider
        .get_game_record()
        .map_err(|e| PCSError::internal_error(ErrorCode::B30_GET_GAME_RECORD, e.to_string()))?;
    let user_info = provider
        .get_user()
        .map_err(|e| PCSError::internal_error(ErrorCode::B30_GET_USER, e.to_string()))?;
    let game_progress = provider
        .get_game_progress()
        .map_err(|e| PCSError::internal_error(ErrorCode::B30_GET_GAME_PROGRESS, e.to_string()))?;
    let settings = provider
        .get_settings()
        .map_err(|e| PCSError::internal_error(ErrorCode::B30_GET_SETTINGS, e.to_string()))?;

    let fetcher = PhiInfoFetcher::new(backend).await?;
    let songs = fetcher.get_songs().await?;
    let song_map: BTreeMap<String, SongInfo> =
        songs.into_iter().map(|s| (s.id.clone(), s)).collect();

    let (p3, b27, total_rks) = compute_b30(&song_map, &game_record.songs);

    // p3（前3个AP）+ b27（前27个成绩），可重叠，共30张卡片
    let mut all_30: Vec<BestPlay> = Vec::with_capacity(30);
    all_30.extend(p3);
    all_30.extend(b27);

    let owned_cards = build_b30_cards(&fetcher, &song_map, &all_30).await?;
    let cards = into_card_data(&owned_cards);

    let utc_time: DateTime<Utc> = gs.updated_at;
    let offset_eight_hours = FixedOffset::east_opt(8 * 3600).unwrap();
    let date_str = utc_time
        .with_timezone(&offset_eight_hours)
        .format("%Y-%m-%d %H:%M")
        .to_string();
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

    template.render().map_err(|e| {
        PCSError::internal_error(
            ErrorCode::TEMPLATE_RENDER,
            format!("template render failed: {}", e),
        )
    })
}
