#![allow(dead_code)]

use alloc::{
    collections::btree_map::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[serde(rename = "zh_cn")]
    ZhCn = 0x28,
    #[serde(rename = "zh_tw")]
    ZhTw = 0x29,
    En = 0x0A,
    Ja = 0x16,
    Ko = 0x17,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SongLevel {
    /// 谱师
    pub charter: String,
    /// 定数
    pub difficulty: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SongInfo {
    pub id: String,
    /// keyStore用的
    pub key: String,
    /// 名称
    pub name: String,
    /// 曲师
    pub composer: String,
    /// 画师
    pub illustrator: String,
    /// 预览时间
    pub preview_time: f64,
    pub preview_end_time: f64,
    /// key=难度等级
    pub levels: BTreeMap<String, SongLevel>,
}

impl SongInfo {
    pub fn ill_low_res_path(&self) -> String {
        format!("Assets/Tracks/{}/IllustrationLowRes.jpg", self.id)
    }

    pub fn ill_path(&self) -> String {
        format!("Assets/Tracks/{}/Illustration.jpg", self.id)
    }

    pub fn ill_blur_path(&self) -> String {
        format!("Assets/Tracks/{}/IllustrationBlur.jpg", self.id)
    }

    pub fn music_path(&self) -> String {
        format!("Assets/Tracks/{}/music.wav", self.id)
    }

    pub fn get_chart_path(&self, difficulty: &str) -> Result<String, String> {
        if !self.levels.contains_key(difficulty) {
            return Err(format!(
                "This song does not have requested difficulty: {}",
                difficulty
            ));
        }
        Ok(format!(
            "Assets/Tracks/{}/Chart_{}.json",
            self.id, difficulty
        ))
    }
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Folder {
    pub title: BTreeMap<Language, String>,
    /// 空字符串时不需要渲染
    pub sub_title: BTreeMap<Language, String>,
    /// 为 addressable_key
    pub cover: String,
    pub files: Vec<FileItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileItem {
    /// keyStore用的
    pub key: String,
    /// 云存档用的
    pub sub_index: i32,
    /// 名称
    pub name: BTreeMap<Language, String>,
    /// 收集时间
    pub date: String,
    /// 保管单位
    pub supervisor: BTreeMap<Language, String>,
    /// 等级
    pub category: String,
    /// 内容
    pub content: BTreeMap<Language, String>,
    /// 额外信息,单个 "名称=值" 结构,与其他信息并列
    pub properties: BTreeMap<Language, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Avatar {
    pub name: String,
    pub addressable_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChapterInfo {
    pub code: String,
    /// 目前无法提取名称,可以用横幅当名称
    pub banner: String,
    pub song_ids: Vec<String>,
}

impl ChapterInfo {
    pub fn cover_blur_path(&self) -> String {
        if self.code == "MainStory8" {
            "Assets/Tracks/#ChapterCover/MainStory8_2BlurS.jpg".to_string()
        } else {
            format!("Assets/Tracks/#ChapterCover/{}Blur.jpg", self.code)
        }
    }

    pub fn cover_path(&self) -> String {
        if self.code == "MainStory8" {
            "Assets/Tracks/#ChapterCover/MainStory8_2S.jpg".to_string()
        } else {
            format!("Assets/Tracks/#ChapterCover/{}.jpg", self.code)
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PhiVersion {
    pub code: u32,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Suffix {
    pub image: String,
    pub text: String,
    pub music: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiInfo {
    pub version: String,
    #[serde(rename = "type")]
    pub api_type: String,
    pub suffix: Suffix,
}
