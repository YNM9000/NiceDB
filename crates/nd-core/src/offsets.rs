pub(crate) fn utf16_to_byte(text: &str, utf16: u32) -> usize {
    let mut units = 0u32;
    for (byte, ch) in text.char_indices() {
        if units >= utf16 {
            return byte;
        }
        units += u32::try_from(ch.len_utf16()).unwrap_or(2);
    }
    text.len()
}

pub(crate) fn byte_to_utf16(text: &str, byte: usize) -> u32 {
    let end = text.floor_char_boundary(byte.min(text.len()));
    u32::try_from(text[..end].encode_utf16().count()).unwrap_or(u32::MAX)
}
