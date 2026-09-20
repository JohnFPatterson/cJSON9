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

    #[test]
    fn hex4_mixed_case_and_invalid() {
        assert_eq!(parse_hex4(b"bEeF"), 0xBEEF);
        assert_eq!(parse_hex4(b"BeeF"), 0xBEEF);
        // Invalid digit yields 0, same as C (does not return a partial value).
        assert_eq!(parse_hex4(b"gggg"), 0);
        assert_eq!(parse_hex4(b"abcg"), 0);
        // Fewer than 4 bytes: 0 (C would read past the pointer).
        assert_eq!(parse_hex4(b"abc"), 0);
        assert_eq!(parse_hex4(b""), 0);
    }

    #[test]
    fn skip_utf8_bom_only_at_offset_zero_and_needs_five_bytes() {
        // C `can_access_at_index(..., 4)` is `offset + 4 < length`.
        let mut offset = 0usize;
        let with_room = [0xEF, 0xBB, 0xBF, b'{', b'}', 0];
        assert!(skip_utf8_bom(&with_room, &mut offset));
        assert_eq!(offset, 3);

        let mut offset = 0usize;
        let too_short = [0xEF, 0xBB, 0xBF, b'{']; // length 4: 0+4 < 4 is false
        assert!(skip_utf8_bom(&too_short, &mut offset));
        assert_eq!(offset, 0);

        let mut offset = 1usize;
        let prefixed = [b' ', 0xEF, 0xBB, 0xBF, b'{', b'}'];
        assert!(!skip_utf8_bom(&prefixed, &mut offset));
        assert_eq!(offset, 1);
    }

    #[test]
    fn skip_whitespace_bytes_le_32_and_step_back_at_end() {
        let mut offset = 0usize;
        skip_whitespace(b" \t\n\r{}", &mut offset);
        assert_eq!(offset, 4);

        let mut offset = 0usize;
        skip_whitespace(b"   ", &mut offset);
        // Walked to length, then stepped back one (cJSON quirk).
        assert_eq!(offset, 2);

        let mut offset = 0usize;
        skip_whitespace(b"", &mut offset);
        assert_eq!(offset, 0);
    }

    /// Bytes `parse_number` copies before `strtod`: digits, `+`, `-`, `e`, `E`, `.`
    const NUMBER_TOKEN_CHARSET: &[u8] = b"0123456789+-eE.";

    #[test]
    fn number_token_charset_is_digits_sign_exp_dot() {
        assert_eq!(NUMBER_TOKEN_CHARSET.len(), 15);
        for b in 0u8..=255 {
            let allowed = NUMBER_TOKEN_CHARSET.contains(&b);
            let expected = matches!(b, b'0'..=b'9' | b'+' | b'-' | b'e' | b'E' | b'.');
            assert_eq!(allowed, expected, "byte {b:#04x}");
        }
    }

    #[test]
    #[ignore = "requires parse() implementation"]
    fn parse_number_token_then_stops_on_other_bytes() {
        // Trailing junk is OK unless require_null_terminated. "1.5e+10," should
        // parse a number; "x1" should fail. Full assertions live in tests/abi_golden.rs.
        let _ = NUMBER_TOKEN_CHARSET;
    }
}
