use serde::{Deserialize, Serialize};

use crate::user::model::Session;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventUser {
    pub openid: String,
    pub session_token: String,
    pub nickname: String,
}

impl EventUser {
    pub fn new(
        openid: impl Into<String>,
        session_token: impl Into<String>,
        nickname: impl Into<String>,
    ) -> Self {
        Self {
            openid: openid.into(),
            session_token: session_token.into(),
            nickname: nickname.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "user.login")]
    UserLogin { user: EventUser },
    #[serde(rename = "user.create")]
    UserCreate { user: EventUser },
    #[serde(rename = "user.update")]
    UserUpdate { user: EventUser },
    #[serde(rename = "user.delete")]
    UserDelete { user: EventUser },
    #[serde(rename = "user.refresh_session_token")]
    UserRefreshSessionToken { user: EventUser },
    #[serde(rename = "save.create")]
    SaveCreate {
        user: EventUser,
        file_object_id: String,
        summary: String,
    },
    #[serde(rename = "save.update")]
    SaveUpdate {
        user: EventUser,
        file_object_id: String,
        summary: String,
    },
}

impl From<&Session> for EventUser {
    fn from(s: &Session) -> Self {
        Self {
            openid: s.openid.clone(),
            session_token: s.session_token.clone(),
            nickname: s.nickname.clone(),
        }
    }
}
