use alloc::string::String;
use askama::Template;

const SONG_NAME_MAX_WIDTH: u32 = 130;

#[derive(Clone, Copy)]
pub enum Difficulty {
    At,
    In,
    Hd,
    Ez,
}

impl Difficulty {
    pub fn rank_bg(&self) -> &'static str {
        match self {
            Self::At => "rgba(110,110,110,1)",
            Self::In => "rgba(255,0,0,1)",
            Self::Hd => "rgba(0,176,240,1)",
            Self::Ez => "rgba(146,208,80,1)",
        }
    }

    pub fn info_bg(&self) -> &'static str {
        match self {
            Self::At => "rgba(240,0,0,0.3)",
            Self::In | Self::Hd | Self::Ez => "rgba(0,183,240,0.3)",
        }
    }

    pub fn info_border(&self) -> &'static str {
        match self {
            Self::At => "rgba(255,0,0,1)",
            Self::In | Self::Hd | Self::Ez => "rgba(0,183,240,1)",
        }
    }
}

pub(crate) struct CardData<'a> {
    pub(crate) index: String,
    pub(crate) name: &'a str,
    pub(crate) name_font_size: u32,
    pub(crate) name_fit_width: Option<u32>,
    pub(crate) illustration_link: String,
    pub(crate) rank: &'a str,
    pub(crate) rks: &'a str,
    pub(crate) rating_link: String,
    pub(crate) score: &'a str,
    pub(crate) acc: &'a str,
    pub(crate) rank_color: &'a str,
    pub(crate) info_bg: &'a str,
    pub(crate) info_border: &'a str,
}

#[derive(Template)]
#[template(path = "b30.svg")]
pub(crate) struct B30Template<'a> {
    pub(crate) bg_link: &'a str,
    pub(crate) icon_link: &'a str,
    pub(crate) challenge_link: &'a str,
    pub(crate) font_link: &'a str,
    pub(crate) player_id: &'a str,
    pub(crate) rks: &'a str,
    pub(crate) challenge_mode_rank: u16,
    pub(crate) data: &'a str,
    pub(crate) device: &'a str,
    pub(crate) date: &'a str,
    pub(crate) cards: &'a [Option<CardData<'a>>],
}

pub(crate) fn estimate_song_name_font_size(name: &str) -> u32 {
    let weighted_em_width = name.chars().map(estimated_char_em_width).sum::<f32>();

    if weighted_em_width <= 0.0 {
        return 14;
    }

    (((SONG_NAME_MAX_WIDTH as f32 / weighted_em_width) + 0.5) as u32).clamp(6, 16)
}

pub(crate) fn estimate_song_name_fit_width(name: &str) -> Option<u32> {
    let font_size = estimate_song_name_font_size(name) as f32;
    let estimated_width = name.chars().map(estimated_char_em_width).sum::<f32>() * font_size;

    if estimated_width > SONG_NAME_MAX_WIDTH as f32 {
        Some(SONG_NAME_MAX_WIDTH)
    } else {
        None
    }
}

fn estimated_char_em_width(ch: char) -> f32 {
    if ch.is_ascii_uppercase() {
        0.62
    } else if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
        0.53
    } else if ch.is_ascii_whitespace() {
        0.33
    } else if ch.is_ascii_punctuation() {
        0.36
    } else {
        1.0
    }
}
