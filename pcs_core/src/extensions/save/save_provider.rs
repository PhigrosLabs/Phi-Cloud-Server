use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use aes::Aes256;
use cbc::Decryptor;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};

use miniz_oxide::inflate::stream::{InflateState, inflate};
use miniz_oxide::{DataFormat, MZFlush};
use tinyzip::{Archive, Compression};

use phi_save_codec::{Binary, GameKey, GameProgress, GameRecord, Settings, User};
type Aes256CbcDec = Decryptor<Aes256>;

pub struct SaveProvider<'a> {
    archive: Archive<&'a [u8]>,
}

impl<'a> SaveProvider<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, tinyzip::Error<tinyzip::SliceReaderError>> {
        let archive = Archive::open(data)?;

        Ok(Self { archive })
    }

    fn get_entry_raw(&self, name: &'static [u8]) -> Result<Vec<u8>, String> {
        let entry = self
            .archive
            .find_file(name)
            .map_err(|e| format!("find_file failed: {:?}", e))?;

        let mut buffer: Vec<u8> = Vec::new();

        match entry
            .compression()
            .map_err(|e| format!("compression error: {:?}", e))?
        {
            Compression::Stored => {
                entry
                    .read_to_slice(&mut buffer)
                    .map_err(|e| format!("read error: {:?}", e))?;
            }

            Compression::Deflated => {
                let mut state = InflateState::new(DataFormat::Raw);
                let mut out = [0u8; 16384];
                let mut out_pos = 0;

                let mut chunks = entry
                    .read_chunks::<512>()
                    .map_err(|e| format!("chunk error: {:?}", e))?;

                while let Some(chunk) = chunks.next() {
                    let chunk = chunk.map_err(|e| format!("chunk read error: {:?}", e))?;

                    let result = inflate(&mut state, &chunk, &mut out[out_pos..], MZFlush::None);

                    out_pos += result.bytes_written;
                }

                buffer.extend_from_slice(&out[..out_pos]);
            }
        }
        Ok(buffer)
    }

    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        const AES_KEY: &[u8; 32] = &[
            0xe8, 0x96, 0x9a, 0xd2, 0xa5, 0x40, 0x25, 0x9b, 0x97, 0x91, 0x90, 0x8b, 0x88, 0xe6,
            0xbf, 0x03, 0x1e, 0x6d, 0x21, 0x95, 0x6e, 0xfa, 0xd6, 0x8a, 0x50, 0xdd, 0x55, 0xd6,
            0x7a, 0xb0, 0x92, 0x4b,
        ];

        const AES_IV: &[u8; 16] = &[
            0x2a, 0x4f, 0xf0, 0x8a, 0xc8, 0x0d, 0x63, 0x07, 0x00, 0x57, 0xc5, 0x95, 0x18, 0xc8,
            0x32, 0x53,
        ];

        let mut buf = data.to_vec();

        let pt = Aes256CbcDec::new(AES_KEY.into(), AES_IV.into())
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map_err(|e| format!("AES decrypt failed: {:?}", e))?;

        Ok(pt.to_vec())
    }

    fn read_decrypted(&self, name: &'static [u8]) -> Result<Vec<u8>, String> {
        let raw = self.get_entry_raw(name)?;

        let (version, body) = raw.split_first().ok_or_else(|| "empty entry".to_string())?;

        let mut decrypted = self.decrypt(body)?;

        decrypted.insert(0, *version);
        Ok(decrypted)
    }

    pub fn get_game_key(&self) -> Result<GameKey, String> {
        let data = self.read_decrypted(b"gameKey")?;
        GameKey::read(&data).map_err(|e| format!("{:?}", e))
    }

    pub fn get_game_record(&self) -> Result<GameRecord, String> {
        let data = self.read_decrypted(b"gameRecord")?;
        GameRecord::read(&data).map_err(|e| format!("{:?}", e))
    }

    pub fn get_game_progress(&self) -> Result<GameProgress, String> {
        let data = self.read_decrypted(b"gameProgress")?;
        GameProgress::read(&data).map_err(|e| format!("{:?}", e))
    }

    pub fn get_settings(&self) -> Result<Settings, String> {
        let data = self.read_decrypted(b"settings")?;
        Settings::read(&data).map_err(|e| format!("{:?}", e))
    }

    pub fn get_user(&self) -> Result<User, String> {
        let data = self.read_decrypted(b"user")?;
        User::read(&data).map_err(|e| format!("{:?}", e))
    }
}
