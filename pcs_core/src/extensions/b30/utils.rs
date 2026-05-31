#![allow(dead_code)]

use alloc::{
    collections::btree_map::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use base64::Engine;
use core::cmp::Ordering;
use phi_save_codec::{
    game_progress::Money,
    game_record::{LevelRecord, SongRecord},
};
use serde::de::DeserializeOwned;

use crate::{
    extensions::b30::{
        asset::*,
        template::{
            CardData, Difficulty, estimate_song_name_fit_width, estimate_song_name_font_size,
        },
        types::*,
    },
    types::{PCSBackend, PCSError},
    utils::MapPCSError,
};

pub fn single_rks(level_record: &LevelRecord, difficulty: f32) -> f32 {
    let t = (level_record.acc - 55.0) / 45.0;
    t * t * difficulty
}

pub fn get_rating_img(level_record: &LevelRecord) -> &'static [u8] {
    if level_record.score == 1_000_000 {
        RATING_PHI
    } else if level_record.fc {
        RATING_FC
    } else if level_record.score >= 960_000 {
        RATING_V
    } else if level_record.score >= 920_000 {
        RATING_S
    } else if level_record.score >= 880_000 {
        RATING_A
    } else if level_record.score >= 820_000 {
        RATING_B
    } else if level_record.score >= 700_000 {
        RATING_C
    } else {
        RATING_F
    }
}

fn get_first_and_remaining(num: u16) -> (u8, u16) {
    let mut n = num;
    let mut divisor = 1;

    while n >= 10 {
        n /= 10;
        divisor *= 10;
    }

    let first_digit = num / divisor; // 54321 / 10000 = 5
    let remaining = num % divisor; // 54321 % 10000 = 4321

    (first_digit as u8, remaining)
}

pub fn get_challenge(challenge: u16) -> (&'static [u8], u16) {
    if challenge >= 600 || challenge == 0 {
        return (CHALLENGE_0, 0);
    }
    let (first, remaining) = get_first_and_remaining(challenge);
    match first {
        0 => (CHALLENGE_0, remaining),
        1 => (CHALLENGE_1, remaining),
        2 => (CHALLENGE_2, remaining),
        3 => (CHALLENGE_3, remaining),
        4 => (CHALLENGE_4, remaining),
        5 => (CHALLENGE_5, remaining),
        _ => (CHALLENGE_0, remaining),
    }
}

pub struct PhiInfoFetcher<'a, T: PCSBackend> {
    provider: &'a T,
    suffix: Suffix,
}

impl<'a, T: PCSBackend> PhiInfoFetcher<'a, T> {
    pub async fn new(provider: &'a T) -> Result<PhiInfoFetcher<'a, T>, PCSError> {
        let response = provider
            .call_phi_info_router("/api_info.json")
            .await
            .map_internal_err()?;
        let api_info: ApiInfo = serde_json::from_slice(&response.data).map_internal_err()?;

        if !api_info.version.starts_with("1.") {
            return Err(PCSError::internal_error(format!(
                "PhiInfoFetcher: API version mismatch! Expected 1.*, got {}",
                api_info.version
            )));
        }

        Ok(Self {
            provider,
            suffix: api_info.suffix,
        })
    }

    async fn fetch_json<R: DeserializeOwned>(&self, path: &str) -> Result<R, PCSError> {
        let response = self
            .provider
            .call_phi_info_router(path)
            .await
            .map_internal_err()?;
        let data: R = serde_json::from_slice(&response.data).map_internal_err()?;
        Ok(data)
    }

    pub fn cached_suffix(&self) -> &Suffix {
        &self.suffix
    }

    pub async fn get_songs(&self) -> Result<Vec<SongInfo>, PCSError> {
        self.fetch_json("/info/songs.json").await
    }

    pub async fn get_collection(&self) -> Result<Vec<Folder>, PCSError> {
        self.fetch_json("/info/collection.json").await
    }

    pub async fn get_avatars(&self) -> Result<Vec<Avatar>, PCSError> {
        self.fetch_json("/info/avatars.json").await
    }

    pub async fn get_tips(&self) -> Result<BTreeMap<Language, Vec<String>>, PCSError> {
        self.fetch_json("/info/tips.json").await
    }

    pub async fn get_chapters(&self) -> Result<Vec<ChapterInfo>, PCSError> {
        self.fetch_json("/info/chapters.json").await
    }

    pub async fn get_version(&self) -> Result<PhiVersion, PCSError> {
        self.fetch_json("/info/version.json").await
    }

    pub async fn get_asset_metadata(&self) -> Result<Vec<String>, PCSError> {
        self.fetch_json("/asset/metadata.json").await
    }

    pub async fn get_image_asset(&self, key: &str) -> Result<(String, Vec<u8>), PCSError> {
        let path = format!("/asset/{}.{}", key, self.suffix.image);
        let response = self
            .provider
            .call_phi_info_router(&path)
            .await
            .map_internal_err()?;
        Ok((response.mime, response.data))
    }

    pub async fn get_music_asset(&self, key: &str) -> Result<(String, Vec<u8>), PCSError> {
        let path = format!("/asset/{}.{}", key, self.suffix.music);
        let response = self
            .provider
            .call_phi_info_router(&path)
            .await
            .map_internal_err()?;
        Ok((response.mime, response.data))
    }

    pub async fn get_text_asset(&self, key: &str) -> Result<(String, Vec<u8>), PCSError> {
        let path = format!("/asset/{}.{}", key, self.suffix.text);
        let response = self
            .provider
            .call_phi_info_router(&path)
            .await
            .map_internal_err()?;
        Ok((response.mime, response.data))
    }
}

pub fn ill_low_res_path(id: &str) -> String {
    format!("Assets/Tracks/{}/IllustrationLowRes.jpg", id)
}

pub fn build_money_string(money: &Money) -> String {
    let mut parts = Vec::with_capacity(5);

    if money.pib.0 != 0 {
        parts.push(format!("{}PiB", money.pib.0));
    }
    if money.tib.0 != 0 {
        parts.push(format!("{}TiB", money.tib.0));
    }
    if money.gib.0 != 0 {
        parts.push(format!("{}GiB", money.gib.0));
    }
    if money.mib.0 != 0 {
        parts.push(format!("{}MiB", money.mib.0));
    }
    if money.kib.0 != 0 {
        parts.push(format!("{}KiB", money.kib.0));
    }

    if parts.is_empty() {
        "0KiB".to_string()
    } else {
        parts.join(" ")
    }
}

fn data_uri(data: (&str, &[u8])) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(data.1);
    format!("data:{};base64,{}", data.0, b64)
}

pub fn asset_data_uri(data: (String, Vec<u8>)) -> String {
    data_uri((&data.0, &data.1))
}

pub fn png_data_uri(bytes: &[u8]) -> String {
    data_uri(("image/png", bytes))
}

pub fn woff2_data_uri(bytes: &[u8]) -> String {
    data_uri(("font/woff2", bytes))
}

pub struct BestPlay {
    pub song_id: String,
    pub difficulty: DiffKind,
    pub level_record: LevelRecord,
    pub rks: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum DiffKind {
    At,
    In,
    Hd,
    Ez,
}

impl DiffKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiffKind::At => "AT",
            DiffKind::In => "IN",
            DiffKind::Hd => "HD",
            DiffKind::Ez => "EZ",
        }
    }

    pub fn to_template(self) -> Difficulty {
        match self {
            DiffKind::At => Difficulty::At,
            DiffKind::In => Difficulty::In,
            DiffKind::Hd => Difficulty::Hd,
            DiffKind::Ez => Difficulty::Ez,
        }
    }
}

pub fn collect_all_levels(
    song_record: &SongRecord,
    song_info: &SongInfo,
) -> Vec<(DiffKind, LevelRecord, f32)> {
    let mut results = Vec::new();
    if let SongRecord::Normal(normal) = song_record {
        let diffs: [(DiffKind, &Option<LevelRecord>); 4] = [
            (DiffKind::At, &normal.at),
            (DiffKind::In, &normal.r#in),
            (DiffKind::Hd, &normal.hd),
            (DiffKind::Ez, &normal.ez),
        ];
        for (kind, opt_level) in &diffs {
            if let Some(level) = opt_level {
                if level.acc < 70.0 {
                    continue;
                }
                if let Some(song_level) = song_info.levels.get(kind.as_str()) {
                    let rks = single_rks(level, song_level.difficulty);
                    results.push((*kind, level.clone(), rks));
                }
            }
        }
    }
    results
}

pub fn compute_b30(
    song_map: &BTreeMap<String, SongInfo>,
    songs: &[(String, SongRecord)],
) -> (Vec<BestPlay>, f32) {
    let mut all_plays: Vec<BestPlay> = Vec::new();

    for (song_id, song_record) in songs {
        let song_info = match song_map.get(song_id) {
            Some(info) => info,
            None => continue,
        };
        for (kind, level_record, rks) in collect_all_levels(song_record, song_info) {
            all_plays.push(BestPlay {
                song_id: song_id.clone(),
                difficulty: kind,
                level_record,
                rks,
            });
        }
    }

    all_plays.sort_by(|a, b| b.rks.partial_cmp(&a.rks).unwrap_or(Ordering::Equal));

    let n = all_plays.len();

    let mut mask = alloc::vec![false; n];
    let mut phi = 0u32;
    for (i, p) in all_plays.iter().enumerate() {
        if phi >= 3 {
            break;
        }
        if p.level_record.score == 1_000_000 {
            mask[i] = true;
            phi += 1;
        }
    }
    for item in mask.iter_mut().take(n.min(27)) {
        *item = true;
    }

    let total_rks = if n == 0 {
        0.0
    } else {
        let sum: f32 = mask
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, m)| *m)
            .map(|(i, _)| all_plays[i].rks)
            .sum();
        sum / 30.0
    };

    let mut selected: Vec<usize> = mask
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, m)| *m)
        .map(|(i, _)| i)
        .collect();
    selected.sort_unstable_by(|a: &usize, b: &usize| b.cmp(a));

    let mut all_30 = Vec::with_capacity(selected.len());
    for idx in selected {
        all_30.push(all_plays.swap_remove(idx));
    }
    all_30.reverse();

    (all_30, total_rks)
}

pub struct PlayCardOwned {
    pub song_name: String,
    pub difficulty: String,
    pub rks: String,
    pub score: String,
    pub acc: String,
    pub ill_uri: String,
    pub rating_uri: String,
    pub diff_kind: DiffKind,
    pub index_str: String,
}

pub async fn build_b30_cards<T: PCSBackend>(
    fetcher: &PhiInfoFetcher<'_, T>,
    song_map: &BTreeMap<String, SongInfo>,
    all_30: &[BestPlay],
) -> Result<Vec<PlayCardOwned>, PCSError> {
    let mut cards = Vec::with_capacity(all_30.len());

    for (i, play) in all_30.iter().enumerate() {
        let song_info = match song_map.get(&play.song_id) {
            Some(info) => info,
            None => continue,
        };

        let ill_uri = asset_data_uri(
            fetcher
                .get_image_asset(&song_info.ill_low_res_path())
                .await
                .map_internal_err()?,
        );

        let rating_uri = png_data_uri(get_rating_img(&play.level_record));

        let index_str = if i < 3 {
            format!("P{}", i + 1)
        } else {
            format!("B{}", i - 3 + 1)
        };

        cards.push(PlayCardOwned {
            song_name: song_info.name.clone(),
            difficulty: format!(
                "{} {}",
                play.difficulty.as_str(),
                song_info
                    .levels
                    .get(play.difficulty.as_str())
                    .map(|sl| format!("{}", sl.difficulty))
                    .unwrap_or_default()
            ),
            rks: format!("{:.2}", play.rks),
            score: format!("{}", play.level_record.score),
            acc: format!("{:.2}%", play.level_record.acc),
            ill_uri,
            rating_uri,
            diff_kind: play.difficulty,
            index_str,
        });
    }

    Ok(cards)
}

pub fn into_card_data(owned: &[PlayCardOwned]) -> Vec<Option<CardData<'_>>> {
    let mut cards: Vec<Option<CardData<'_>>> = Vec::with_capacity(30);

    for pc in owned {
        let diff = pc.diff_kind.to_template();
        cards.push(Some(CardData {
            index: pc.index_str.clone(),
            name: &pc.song_name,
            name_font_size: estimate_song_name_font_size(&pc.song_name),
            name_fit_width: estimate_song_name_fit_width(&pc.song_name),
            illustration_link: pc.ill_uri.clone(),
            rank: &pc.difficulty,
            rks: &pc.rks,
            rating_link: pc.rating_uri.clone(),
            score: &pc.score,
            acc: &pc.acc,
            rank_color: diff.rank_bg(),
            info_bg: diff.info_bg(),
            info_border: diff.info_border(),
        }));
    }

    while cards.len() < 30 {
        cards.push(None);
    }

    cards
}

pub struct PersonalInfo {
    pub bg_uri: String,
    pub icon_uri: String,
    pub challenge_uri: String,
    pub font_uri: String,
    pub rks: String,
    pub data: String,
    pub device: String,
    pub date: String,
    pub challenge_mode_rank: u16,
}

pub async fn build_personal_info<T: PCSBackend>(
    fetcher: &PhiInfoFetcher<'_, T>,
    user_bg_id: &str,
    challenge_mode_rank: u16,
    money: &Money,
    device_name: &str,
    date_str: &str,
    total_rks: f32,
) -> Result<PersonalInfo, PCSError> {
    let bg_uri = asset_data_uri(
        fetcher
            .get_image_asset(&ill_low_res_path(user_bg_id))
            .await
            .map_internal_err()?,
    );
    let icon_uri = png_data_uri(OTHER_ICON);
    let (challenge_bytes, challenge_rank) = get_challenge(challenge_mode_rank);
    let challenge_uri = png_data_uri(challenge_bytes);
    let font_uri = woff2_data_uri(OTHER_FONT_WOFF2);

    Ok(PersonalInfo {
        bg_uri,
        icon_uri,
        challenge_uri,
        font_uri,
        rks: format!("{:.6}", total_rks),
        data: build_money_string(money),
        device: device_name.to_string(),
        date: date_str.to_string(),
        challenge_mode_rank: challenge_rank,
    })
}
