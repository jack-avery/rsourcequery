use crate::error::SourceQueryError;

/// Get the value of a null-terminated string
/// with index 0 at `offset` in an array of bytes.
/// 
/// Mutates `offset` to the index after the null-termination byte.
pub fn get_string(data: &[u8], offset: &mut usize) -> Result<String, SourceQueryError> {
    let start_offset: usize = *offset;
    let mut end_offset: usize = *offset;

    while let Some(c) = data.get(end_offset) {
        end_offset += 1;
        if c == &0u8 {
            break;
        }
    }
    *offset = end_offset;

    Ok(std::str::from_utf8(&data[start_offset..end_offset-1])?.to_string())
}

/// Get the [u8] at index `offset` from `data`.
/// 
/// Mutates `offset` to the index after the byte.
pub fn get_u8(data: &[u8], offset: &mut usize) -> u8 {
    let byte: u8 = data[*offset];
    *offset += 1;
    byte
}

/// Get 2 bytes (as a [u16]) at index `offset` from `data`.
/// 
/// Mutates `offset` to the index after the bytes.
pub fn get_u16(data: &[u8], offset: &mut usize) -> u16 {
    let bytes: &[u8] = &data[*offset..=*offset + 1];
    *offset += 2;
    ((bytes[1] as u16) << 8) | (bytes[0] as u16)
}

/// Get 8 bytes (as a [u64]) at index `offset` from `data`.
/// 
/// Mutates `offset` to the index after the bytes.
pub fn get_u64(data: &[u8], offset: &mut usize) -> u64 {
    let bytes: &[u8] = &data[*offset..*offset + 8];
    *offset += 8;
    ((bytes[7] as u64) << 56) |
    ((bytes[6] as u64) << 48) |
    ((bytes[5] as u64) << 40) |
    ((bytes[4] as u64) << 32) |
    ((bytes[3] as u64) << 24) |
    ((bytes[2] as u64) << 16) |
    ((bytes[1] as u64) << 8) |
    (bytes[0] as u64)
}