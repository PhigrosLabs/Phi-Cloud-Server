use alloc::{string::String, vec::Vec};

use crate::types::ByteStream;

pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub body: Vec<u8>,
    // header: X-LC-Session
    pub session_token: Option<&'a str>,
    // http(s)://{host}:{port}
    pub server_url: &'a str,
}

pub enum Body<T: ByteStream> {
    Bytes(Vec<u8>),
    ByteStream(T),
}

pub struct Response<T: ByteStream> {
    pub status_code: u16,
    pub content_type: Option<String>,
    pub body: Option<Body<T>>,
}
