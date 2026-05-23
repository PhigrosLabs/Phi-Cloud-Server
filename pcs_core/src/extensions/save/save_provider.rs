use core::convert::Infallible;
use core::fmt;

use aes::cipher::BlockEncryptMut;
use alloc::vec::Vec;

use aes::Aes256;
use cbc::Decryptor;
use cbc::Encryptor;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use phi_save_codec::Binary;
use phi_save_codec::{GameKey, GameProgress, GameRecord, Settings, User};
use shua_zip::{ReadAt, ZipArchive, ZipError};

type Aes256CbcDec = Decryptor<Aes256>;
type Aes256CbcEnc = Encryptor<Aes256>;

#[derive(Debug)]
pub enum SaveError {
    Zip(ZipError<Infallible>),
    FileNotFound(&'static str),
    EmptyEntry(&'static str),
    DecryptionFailed,
    CodecFailed,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Zip(e) => write!(f, "Zip archive error: {:?}", e),
            SaveError::FileNotFound(name) => write!(f, "File not found in archive: {}", name),
            SaveError::EmptyEntry(name) => write!(f, "File entry is empty: {}", name),
            SaveError::DecryptionFailed => {
                write!(f, "AES-256-CBC decryption or PKCS7 unpadding failed")
            }
            SaveError::CodecFailed => {
                write!(f, "Failed to parse game data structure (Codec error)")
            }
        }
    }
}

impl From<ZipError<Infallible>> for SaveError {
    fn from(err: ZipError<Infallible>) -> Self {
        SaveError::Zip(err)
    }
}

pub struct SliceReader<'a> {
    data: &'a [u8],
}

impl<'a> SliceReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl<'a> ReadAt for SliceReader<'a> {
    type Error = Infallible;

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), Self::Error> {
        let offset = offset as usize;
        let end = offset + buf.len();
        buf.copy_from_slice(&self.data[offset..end]);
        Ok(())
    }

    fn size(&self) -> u64 {
        self.data.len() as u64
    }
}

const AES_KEY: &[u8; 32] = &[
    0xe8, 0x96, 0x9a, 0xd2, 0xa5, 0x40, 0x25, 0x9b, 0x97, 0x91, 0x90, 0x8b, 0x88, 0xe6, 0xbf, 0x03,
    0x1e, 0x6d, 0x21, 0x95, 0x6e, 0xfa, 0xd6, 0x8a, 0x50, 0xdd, 0x55, 0xd6, 0x7a, 0xb0, 0x92, 0x4b,
];

const AES_IV: &[u8; 16] = &[
    0x2a, 0x4f, 0xf0, 0x8a, 0xc8, 0x0d, 0x63, 0x07, 0x00, 0x57, 0xc5, 0x95, 0x18, 0xc8, 0x32, 0x53,
];

pub struct SaveProvider<'a> {
    reader: Option<SliceReader<'a>>,
    archive: ZipArchive,
}

macro_rules! save_fields {
    (
        $(
            $key:expr =>
            $get:ident,
            $set:ident :
            $ty:ty =>
            $file:expr
        ),* $(,)?
    ) => {
        impl<'a> SaveProvider<'a> {

            $(
                pub fn $get(&self) -> Result<$ty, SaveError> {
                    self.read_decrypted($file)
                }

                pub fn $set(&mut self, value: &$ty) -> Result<(), SaveError> {
                    self.write_encrypted($file, value)
                }
            )*
        }
    };
}

impl<'a> SaveProvider<'a> {
    pub fn new() -> Self {
        Self {
            reader: None,
            archive: ZipArchive::new(),
        }
    }

    pub fn parse(data: &'a [u8]) -> Result<Self, SaveError> {
        let reader = SliceReader::new(data);
        let archive = ZipArchive::new().with_binary(&reader)?;
        Ok(Self {
            reader: Some(reader),
            archive,
        })
    }

    pub fn build(&self) -> Result<Vec<u8>, SaveError> {
        match &self.reader {
            Some(reader) => self.archive.build_with_readat(reader).map_err(Into::into),
            None => self.archive.build().map_err(Into::into),
        }
    }

    fn get_entry_raw(&self, name: &'static str) -> Result<Vec<u8>, SaveError> {
        let index = self
            .archive
            .find_by_name(name)
            .ok_or(SaveError::FileNotFound(name))?;

        let reader = self.reader.as_ref().ok_or(SaveError::EmptyEntry(
            "No data source provided to read from",
        ))?;
        let buffer = self.archive.read_file(index, reader)?;
        Ok(buffer)
    }

    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, SaveError> {
        let mut buf = data.to_vec();

        let pt = Aes256CbcDec::new(AES_KEY.into(), AES_IV.into())
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map_err(|_| SaveError::DecryptionFailed)?;

        Ok(pt.to_vec())
    }

    fn read_decrypted<T: Binary>(&self, name: &'static str) -> Result<T, SaveError> {
        let raw = self.get_entry_raw(name)?;
        let (version, body) = raw.split_first().ok_or(SaveError::EmptyEntry(name))?;
        let mut decrypted = self.decrypt(body)?;
        decrypted.insert(0, *version);
        T::read(&decrypted).map_err(|_| SaveError::CodecFailed)
    }

    fn write_encrypted<T: Binary>(
        &mut self,
        name: &'static str,
        value: &T,
    ) -> Result<(), SaveError> {
        let mut raw_data = alloc::vec![0u8; value.len()];
        value
            .write(&mut raw_data)
            .map_err(|_| SaveError::CodecFailed)?;

        let (version, body) = raw_data.split_first().ok_or(SaveError::EmptyEntry(name))?;

        let mut encrypt_buf = alloc::vec![0u8; body.len() + 16];
        encrypt_buf[..body.len()].copy_from_slice(body);

        let ct = Aes256CbcEnc::new(AES_KEY.into(), AES_IV.into())
            .encrypt_padded_mut::<Pkcs7>(&mut encrypt_buf, body.len())
            .map_err(|_| SaveError::DecryptionFailed)?;
        let mut final_payload = Vec::with_capacity(1 + ct.len());
        final_payload.push(*version);
        final_payload.extend_from_slice(ct);
        if let Some(index) = self.archive.find_by_name(name) {
            self.archive.remove_file(index);
        }
        self.archive.add_file(name, final_payload);

        Ok(())
    }
}

save_fields! {
    "game_key" =>
        get_game_key,
        set_game_key :
        GameKey => "gameKey",

    "game_record" =>
        get_game_record,
        set_game_record :
        GameRecord => "gameRecord",

    "game_progress" =>
        get_game_progress,
        set_game_progress :
        GameProgress => "gameProgress",

    "settings" =>
        get_settings,
        set_settings :
        Settings => "settings",

    "user" =>
        get_user,
        set_user :
        User => "user",
}
