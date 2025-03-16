use hex::FromHexError;

use crate::error::{ApiError, ApiResult};

pub struct HueStreamKey {
    key: [u8; Self::BYTE_SIZE],
}

impl HueStreamKey {
    const BYTE_SIZE: usize = 16;
    const HEX_SIZE: usize = Self::BYTE_SIZE * 2;

    #[must_use]
    pub const fn new(key: [u8; Self::BYTE_SIZE]) -> Self {
        Self { key }
    }

    pub fn write_to_slice(&self, out: &mut [u8]) -> ApiResult<()> {
        if out.len() < Self::BYTE_SIZE {
            return Err(ApiError::FromHexError(FromHexError::InvalidStringLength));
        }
        out[..Self::BYTE_SIZE].copy_from_slice(&self.key);
        Ok(())
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.key)
    }

    pub fn to_hex_slice(&self, out: &mut [u8]) -> ApiResult<()> {
        if out.len() < Self::HEX_SIZE {
            return Err(ApiError::FromHexError(FromHexError::InvalidStringLength));
        }
        Ok(hex::encode_to_slice(self.key, &mut out[..Self::HEX_SIZE])?)
    }
}

impl AsRef<[u8]> for HueStreamKey {
    fn as_ref(&self) -> &[u8] {
        &self.key
    }
}

impl TryFrom<&str> for HueStreamKey {
    type Error = ApiError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut key = [0u8; 16];
        if value.len() < Self::HEX_SIZE {
            return Err(ApiError::FromHexError(FromHexError::InvalidStringLength));
        }

        hex::decode_to_slice(value, &mut key)?;

        Ok(Self::new(key))
    }
}
