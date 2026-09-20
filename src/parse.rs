#![allow(dead_code)]
//! Safe JSON parser matching cJSON 1.7.19 quirks.
//!
//! Implement this module without `unsafe`. The shim supplies a [`TreeBuilder`].

use crate::traits::{ParseError, ParseOptions, TreeBuilder};
use crate::types::CJSON_NESTING_LIMIT;

/// Parse 4 hex digits (cJSON `parse_hex4`). Invalid digits yield 0, same as C.
pub fn parse_hex4(input: &[u8]) -> u32 {
    if input.len() < 4 {
        return 0;
    }
    let mut h: u32 = 0;
    for i in 0..4 {
        let c = input[i];
        h += match c {
            b'0'..=b'9' => u32::from(c - b'0'),
            b'A'..=b'F' => 10 + u32::from(c - b'A'),
            b'a'..=b'f' => 10 + u32::from(c - b'a'),
            _ => return 0,
        };
        if i < 3 {
            h <<= 4;
        }
    }
    h
}

/// Skip bytes `<= 32`. If this walks to `length`, step back one byte (cJSON quirk).
pub fn skip_whitespace(input: &[u8], offset: &mut usize) {
    if input.is_empty() {
        return;
    }
    while *offset < input.len() && input[*offset] <= 32 {
        *offset += 1;
    }
    if *offset == input.len() && !input.is_empty() {
        *offset -= 1;
    }
}

/// Skip UTF-8 BOM only when `offset == 0`. Requires `can_access_at_index(..., 4)`
/// which is `offset + 4 < length` (cJSON quirk: needs 5 bytes of buffer, not 3).
pub fn skip_utf8_bom(input: &[u8], offset: &mut usize) -> bool {
    if *offset != 0 {
        return false;
    }
    if offset.saturating_add(4) < input.len()
        && input.len() >= 3
        && input[0] == 0xEF
        && input[1] == 0xBB
        && input[2] == 0xBF
    {
        *offset += 3;
    }
    true
}

/// Parse a complete JSON value from `input`.
///
/// On success, returns `(root, end_offset)` where `end_offset` is the first unused
/// byte (cJSON `buffer_at_offset` after the value, without requiring NUL).
///
/// Must match cJSON: trailing junk is allowed unless `require_null_terminated`;
/// whitespace is any byte `<= 32`; nesting limit [`CJSON_NESTING_LIMIT`].
pub fn parse<B: TreeBuilder>(
    builder: &mut B,
    input: &[u8],
    opts: ParseOptions,
) -> Result<(B::Handle, usize), ParseError> {
    let _ = (builder, CJSON_NESTING_LIMIT, opts, input);
    // Filled in by the parse port. Returning offset 0 matches an empty-input failure.
    Err(ParseError { offset: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex4_lower_and_upper() {
        assert_eq!(parse_hex4(b"beef"), 0xBEEF);
        assert_eq!(parse_hex4(b"BEEF"), 0xBEEF);
        assert_eq!(parse_hex4(b"beEF"), 0xBEEF);
        assert_eq!(parse_hex4(b"0000"), 0);
        assert_eq!(parse_hex4(b"ffff"), 0xFFFF);
    }
}
