#![allow(dead_code)]
//!
//! Strips space/tab/CR/LF, `//` line comments, and `/* */` comments.
//! Preserves string contents including escapes. Unclosed `/*` yields empty output.
//! Input is treated as a writable C string: rewrite in place and NUL-terminate.

/// Minify `json` in place. Returns the new length (not including the NUL).
/// `json` must include room for a trailing NUL at `json.len() - 1` if it is a
/// C string view; callers typically pass `slice_from_raw_parts_mut` including NUL.
pub fn minify_in_place(json: &mut [u8]) -> usize {
    if json.is_empty() {
        return 0;
    }

    let mut src = 0;
    let mut dst = 0;

    while src < json.len() && json[src] != 0 {
        match json[src] {
            b' ' | b'\t' | b'\r' | b'\n' => {
                src += 1;
            }
            b'/' => {
                let next = peek(json, src + 1);
                if next == b'/' {
                    skip_oneline_comment(json, &mut src);
                } else if next == b'*' {
                    skip_multiline_comment(json, &mut src);
                } else {
                    // C skips a lone `/` without copying it.
                    src += 1;
                }
            }
            b'"' => {
                minify_string(json, &mut src, &mut dst);
            }
            c => {
                json[dst] = c;
                src += 1;
                dst += 1;
            }
        }
    }

    if dst < json.len() {
        json[dst] = 0;
    }
    dst
}

fn peek(json: &[u8], index: usize) -> u8 {
    json.get(index).copied().unwrap_or(0)
}

/// `skip_oneline_comment`: consume `//`, then bytes until `\n` (consumed) or NUL.
fn skip_oneline_comment(json: &[u8], src: &mut usize) {
    *src = src.saturating_add(2);
    while *src < json.len() && json[*src] != 0 {
        if json[*src] == b'\n' {
            *src += 1;
            return;
        }
        *src += 1;
    }
}

/// `skip_multiline_comment`: consume `/*`, then bytes until `*/` or NUL.
/// If the comment is never closed, `src` lands on NUL and `dst` is left as-is
/// (empty output when the comment started at the beginning of the buffer).
fn skip_multiline_comment(json: &[u8], src: &mut usize) {
    *src = src.saturating_add(2);
    while *src < json.len() && json[*src] != 0 {
        if json[*src] == b'*' && peek(json, *src + 1) == b'/' {
            *src = src.saturating_add(2);
            return;
        }
        *src += 1;
    }
}

/// Copy a JSON string including the quotes. `\"` copies both the backslash and
/// the quote (cJSON `minify_string` quirk); a pending `"\\` at end is left as-is.
fn minify_string(json: &mut [u8], src: &mut usize, dst: &mut usize) {
    json[*dst] = json[*src];
    *src += 1;
    *dst += 1;

    while *src < json.len() && json[*src] != 0 {
        json[*dst] = json[*src];

        if json[*src] == b'"' {
            json[*dst] = b'"';
            *src += 1;
            *dst += 1;
            return;
        } else if json[*src] == b'\\' && peek(json, *src + 1) == b'"' {
            let quote = peek(json, *src + 1);
            if *dst + 1 < json.len() {
                json[*dst + 1] = quote;
            }
            *src += 1;
            *dst += 1;
        }

        *src += 1;
        *dst += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::minify_in_place;

    fn minify_to_string(input: &str) -> String {
        let mut buf = Vec::from(input.as_bytes());
        buf.push(0);
        let n = minify_in_place(&mut buf);
        String::from_utf8(buf[..n].to_vec()).expect("minified output is UTF-8")
    }

    fn minify_bytes(input: &[u8]) -> Vec<u8> {
        let mut buf = input.to_vec();
        if buf.last().copied() != Some(0) {
            buf.push(0);
        }
        let n = minify_in_place(&mut buf);
        buf[..n].to_vec()
    }

    #[test]
    fn empty_slice_returns_zero() {
        assert_eq!(minify_in_place(&mut []), 0);
        assert_eq!(minify_to_string(""), "");
    }

    #[test]
    fn unclosed_multiline_comment_yields_empty() {
        // tests/minify_tests.c: cjson_minify_should_not_overflow_buffer
        assert_eq!(minify_to_string("/* bla"), "");
        assert_eq!(minify_to_string("/*"), "");
        assert_eq!(minify_to_string("/* unterminated"), "");
    }

    #[test]
    fn pending_escape_at_end_is_left_intact() {
        // tests/minify_tests.c: pending_escape[] = "\"\\"
        assert_eq!(minify_bytes(b"\"\\\0"), b"\"\\");
        assert_eq!(minify_to_string("\"\\"), "\"\\");
    }

    #[test]
    fn removes_single_line_comments() {
        // tests/minify_tests.c: cjson_minify_should_remove_single_line_comments
        let input = "{// this is {} \"some kind\" of [] comment /*, don't you see\n}";
        assert_eq!(minify_to_string(input), "{}");
    }

    #[test]
    fn consumes_newline_after_line_comment() {
        assert_eq!(minify_to_string("1//c\n2"), "12");
        assert_eq!(minify_to_string("// only comment"), "");
        assert_eq!(minify_to_string("// no newline"), "");
    }

    #[test]
    fn removes_spaces_tabs_cr_lf() {
        // tests/minify_tests.c: cjson_minify_should_remove_spaces
        assert_eq!(minify_to_string("{ \"key\":\ttrue\r\n    }"), "{\"key\":true}");
    }

    #[test]
    fn removes_multiline_comments() {
        // tests/minify_tests.c: cjson_minify_should_remove_multiline_comments
        let input = "{/* this is\n a /* multi\n //line \n {comment \"\\\" */}";
        assert_eq!(minify_to_string(input), "{}");
    }

    #[test]
    fn does_not_modify_strings() {
        // tests/minify_tests.c: cjson_minify_should_not_modify_strings
        let input = "\"this is a string \\\" \\t bla\"";
        assert_eq!(minify_to_string(input), input);
    }

    #[test]
    fn escaped_quote_copies_backslash_and_quote() {
        assert_eq!(minify_to_string("\"foo\\\"bar\""), "\"foo\\\"bar\"");
        assert_eq!(minify_to_string("\"\\\"\""), "\"\\\"\"");
    }

    #[test]
    fn minifies_full_json_with_comments() {
        // tests/minify_tests.c: cjson_minify_should_minify_json
        let input = "{\n\
            \"glossary\": { // comment\n\
                \"title\": \"example glossary\",\n\
          /* multi\n\
         line */\n\
        \t\t\"GlossDiv\": {\n\
                    \"title\": \"S\",\n\
        \t\t\t\"GlossList\": {\n\
                        \"GlossEntry\": {\n\
                            \"ID\": \"SGML\",\n\
        \t\t\t\t\t\"SortAs\": \"SGML\",\n\
        \t\t\t\t\t\"Acronym\": \"SGML\",\n\
        \t\t\t\t\t\"Abbrev\": \"ISO 8879:1986\",\n\
        \t\t\t\t\t\"GlossDef\": {\n\
        \t\t\t\t\t\t\"GlossSeeAlso\": [\"GML\", \"XML\"]\n\
                            },\n\
        \t\t\t\t\t\"GlossSee\": \"markup\"\n\
                        }\n\
                    }\n\
                }\n\
            }\n\
        }";
        let expected = concat!(
            "{",
            "\"glossary\":{",
            "\"title\":\"example glossary\",",
            "\"GlossDiv\":{",
            "\"title\":\"S\",",
            "\"GlossList\":{",
            "\"GlossEntry\":{",
            "\"ID\":\"SGML\",",
            "\"SortAs\":\"SGML\",",
            "\"Acronym\":\"SGML\",",
            "\"Abbrev\":\"ISO 8879:1986\",",
            "\"GlossDef\":{",
            "\"GlossSeeAlso\":[\"GML\",\"XML\"]",
            "},",
            "\"GlossSee\":\"markup\"",
            "}",
            "}",
            "}",
            "}",
            "}"
        );
        assert_eq!(minify_to_string(input), expected);
    }

    #[test]
    fn lone_slash_is_skipped_not_copied() {
        // tests/minify_tests.c: cjson_minify_should_not_loop_infinitely
        // C skips `/` that is neither `//` nor `/*`, without writing it.
        let mut string = vec![b'8', b' ', b'/', b' ', b'5', b'\n', 0];
        let n = minify_in_place(&mut string);
        assert_eq!(&string[..n], b"85");
        assert_eq!(string[n], 0);
    }

    #[test]
    fn unclosed_comment_after_content_keeps_prior_output() {
        assert_eq!(minify_to_string("abc/*"), "abc");
        assert_eq!(minify_to_string("1 /* x"), "1");
    }

    #[test]
    fn nul_terminates_and_reports_length_excluding_nul() {
        let mut buf = b"{ }\0".to_vec();
        let n = minify_in_place(&mut buf);
        assert_eq!(n, 2);
        assert_eq!(&buf[..n], b"{}");
        assert_eq!(buf[n], 0);
    }
}
