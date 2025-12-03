use alloc::{borrow::ToOwned, string::String, vec};
use alloy_primitives::{I256, U256};

pub fn parse_utf8_string(data: &[u8]) -> Result<String, &'static str> {
    String::from_utf8(data.to_owned()).map_err(|_| "Invalid UTF-8 in custom type")
}

pub fn parse_u64(data: &[u8]) -> Result<u64, &'static str> {
    if data.len() > 8 {
        return Err("data len should be <= 8");
    }
    let mut buf = [0u8; 8];
    buf[8 - data.len()..].copy_from_slice(data);
    Ok(u64::from_be_bytes(buf))
}

pub fn parse_u16(data: &[u8]) -> Result<u16, &'static str> {
    if data.len() != 2 {
        return Err("data len should be 2");
    }
    let bytes = [data[0], data[1]];
    Ok(u16::from_be_bytes(bytes))
}

// if value is negative, then it must be 16 bytes with sign extension
pub fn parse_i128(data: &[u8], size: usize) -> Result<i128, &'static str> {
    if data.len() > size {
        return Err("i128 len should be <= 16");
    }
    let mut pad = vec![0u8; size - data.len()];
    pad.extend_from_slice(data);
    let data = pad.as_slice();

    let sign = if data[0] & 0x80 != 0 { 0xFF } else { 0x00 };
    let mut buf = [sign; 16];
    buf[16 - data.len()..].copy_from_slice(data);
    Ok(i128::from_be_bytes(buf))
}

pub fn parse_u128(data: &[u8]) -> Result<u128, &'static str> {
    if data.len() > 16 {
        return Err("u128 len should be <= 16");
    }
    let mut buf = [0u8; 16];
    buf[16 - data.len()..].copy_from_slice(data);
    Ok(u128::from_be_bytes(buf))
}

// if value is negative, then it must be 32 bytes with sign extension
pub fn parse_i256(data: &[u8], size: usize) -> Result<I256, &'static str> {
    if data.len() > size {
        return Err("i256 len should be <= 32");
    }
    let mut pad = vec![0u8; size - data.len()];
    pad.extend_from_slice(data);
    let data = pad.as_slice();

    let sign = if data[0] & 0x80 != 0 { 0xFF } else { 0x00 };
    let mut buf = [sign; 32];
    buf[32 - data.len()..].copy_from_slice(data);
    Ok(I256::from_be_bytes(buf))
}

pub fn parse_u256(data: &[u8]) -> Result<U256, &'static str> {
    if data.len() > 32 {
        return Err("u256 len should be <= 32");
    }
    let mut buf = [0; 32];
    buf[32 - data.len()..].copy_from_slice(data);
    Ok(U256::from_be_bytes(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::hex;

    #[test]
    fn test_parse_i128() {
        let data = hex::decode("ffffffffffffffffffffffffffffff80").unwrap();
        assert_eq!(parse_i128(&data, 16).unwrap(), -128);
        let data = hex::decode("80").unwrap();
        assert_eq!(parse_i128(&data, 16).unwrap(), 128);

        let data = hex::decode("ffffffffffffffc0").unwrap();
        assert_eq!(parse_i128(&data, 8).unwrap(), -64);
        let data = hex::decode("40").unwrap();
        assert_eq!(parse_i128(&data, 8).unwrap(), 64);

        let data = hex::decode("ffffffe0").unwrap();
        assert_eq!(parse_i128(&data, 4).unwrap(), -32);
        let data = hex::decode("20").unwrap();
        assert_eq!(parse_i128(&data, 4).unwrap(), 32);

        let data = hex::decode("fff0").unwrap();
        assert_eq!(parse_i128(&data, 2).unwrap(), -16);
        let data = hex::decode("10").unwrap();
        assert_eq!(parse_i128(&data, 2).unwrap(), 16);

        let data = hex::decode("f8").unwrap();
        assert_eq!(parse_i128(&data, 1).unwrap(), -8);
        let data = hex::decode("08").unwrap();
        assert_eq!(parse_i128(&data, 1).unwrap(), 8);
    }

    #[test]
    fn test_parse_i256() {
        let data = hex::decode("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00")
            .unwrap();
        assert_eq!(parse_i256(&data, 32).unwrap().as_i64(), -256);
    }
}
