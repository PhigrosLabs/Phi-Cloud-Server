use serde::{Deserialize, Serialize};

use crate::{user::model::Session, utils::ToRfc3339Z};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthData {
    pub openid: String,
    pub name: String,
    pub kid: Option<String>,
    pub mac_key: Option<String>,
}

impl AuthData {
    pub fn new(openid: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            openid: openid.into(),
            name: name.into(),
            kid: None,
            mac_key: None,
        }
    }

    pub fn with_kid(mut self, kid: impl Into<String>) -> Self {
        self.kid = Some(kid.into());
        self
    }

    pub fn with_mac_key(mut self, mac_key: impl Into<String>) -> Self {
        self.mac_key = Some(mac_key.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserParams {
    pub nickname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    #[serde(rename = "sessionToken")]
    pub session_token: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
    pub nickname: String,
    #[serde(rename = "shortId")]
    pub short_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    #[serde(rename = "objectId")]
    pub object_id: String,
    pub nickname: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

impl From<&Session> for SessionResponse {
    fn from(s: &Session) -> Self {
        Self {
            session_token: s.session_token.clone(),
            object_id: s.object_id.clone(),
            nickname: s.nickname.clone(),
            short_id: s.short_id.clone(),
            created_at: s.created_at.to_rfc3339_z(),
            updated_at: s.updated_at.to_rfc3339_z(),
        }
    }
}
