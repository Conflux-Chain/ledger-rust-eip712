use alloc::{borrow::ToOwned, string::String};
use alloy_primitives::{I256, U256};

pub fn parse_utf8_string(data: &[u8]) -> Result<String, &'static str> {
    String::from_utf8(data.to_owned()).map_err(|_| "Invalid UTF-8 in custom type")
}

pub fn parse_u64(data: &[u8]) -> Result<u64, &'static str> {
    if data.len() > 8 {
        return Err("invalid data len");
    }
    let mut buf = [0u8; 8];
    buf[8 - data.len()..].copy_from_slice(data);
    Ok(u64::from_be_bytes(buf))
}

pub fn parse_u16(data: &[u8]) -> Result<u16, &'static str> {
    if data.len() != 2 {
        return Err("invalid data len");
    }
    let bytes = [data[0], data[1]];
    Ok(u16::from_be_bytes(bytes))
}

pub fn parse_i128(data: &[u8]) -> Result<i128, &'static str> {
    if data.len() > 16 {
        return Err("invalid data len");
    }
    let sign = if data[0] & 0x80 != 0 { 0xFF } else { 0x00 };
    let mut buf = [sign; 16];
    buf[16 - data.len()..].copy_from_slice(data);
    Ok(i128::from_be_bytes(buf))
}

pub fn parse_u128(data: &[u8]) -> Result<u128, &'static str> {
    if data.len() > 16 {
        return Err("invalid data len");
    }
    let mut buf = [0u8; 16];
    buf[16 - data.len()..].copy_from_slice(data);
    Ok(u128::from_be_bytes(buf))
}

pub fn parse_i256(data: &[u8]) -> Result<I256, &'static str> {
    if data.len() > 32 {
        return Err("invalid data len");
    }
    let sign = if data[0] & 0x80 != 0 { 0xFF } else { 0x00 };
    let mut buf = [sign; 32];
    buf[32 - data.len()..].copy_from_slice(data);
    Ok(I256::from_be_bytes(buf))
}

pub fn parse_u256(data: &[u8]) -> Result<U256, &'static str> {
    if data.len() > 32 {
        return Err("invalid data len");
    }
    let mut buf = [0; 32];
    buf[32 - data.len()..].copy_from_slice(data);
    Ok(U256::from_be_bytes(buf))
}
