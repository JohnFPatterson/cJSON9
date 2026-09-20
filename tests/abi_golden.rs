//! Golden tests against the public C ABI (`cJSON.h`).

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

// Pull in the rlib so #[no_mangle] C ABI symbols are linked.
use cjson as _;

#[allow(non_camel_case_types)]
enum cJSON {}

extern "C" {
    fn cJSON_Parse(value: *const c_char) -> *mut cJSON;
    fn cJSON_Delete(item: *mut cJSON);
    fn cJSON_IsString(item: *const cJSON) -> c_int;
    fn cJSON_IsNumber(item: *const cJSON) -> c_int;
    fn cJSON_IsObject(item: *const cJSON) -> c_int;
    fn cJSON_GetStringValue(item: *const cJSON) -> *mut c_char;
    fn cJSON_GetNumberValue(item: *const cJSON) -> f64;
    fn cJSON_PrintUnformatted(item: *const cJSON) -> *mut c_char;
    fn cJSON_Minify(json: *mut c_char);
    fn cJSON_free(object: *mut std::os::raw::c_void);
}

fn parse(input: &str) -> *mut cJSON {
    let c = CString::new(input).expect("no interior NUL");
    unsafe { cJSON_Parse(c.as_ptr()) }
}

unsafe fn take_c_string(ptr: *mut c_char) -> String {
    assert!(!ptr.is_null());
    let s = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .expect("utf-8")
        .to_owned();
    unsafe { cJSON_free(ptr as *mut _) };
    s
}

#[test]
fn parse_string_escapes_quote_backslash_slash_and_controls() {
    let item = parse(r#""\"\\\/\b\f\n\r\t""#);
    assert!(!item.is_null(), "parse string with JSON escapes");
    unsafe {
        assert!(cJSON_IsString(item) != 0);
        let valuestring = CStr::from_ptr(cJSON_GetStringValue(item))
            .to_string_lossy()
            .into_owned();
        assert_eq!(valuestring, "\"\\/\u{0008}\u{000c}\n\r\t");
        cJSON_Delete(item);
    }
}

#[test]
fn parse_unicode_escape_and_surrogate_pair() {
    let euro = parse(r#""\u20AC""#);
    assert!(!euro.is_null());
    unsafe {
        let s = CStr::from_ptr(cJSON_GetStringValue(euro)).to_string_lossy();
        assert_eq!(s, "€");
        cJSON_Delete(euro);
    }

    let cat = parse(r#""\uD83D\udc31""#);
    assert!(!cat.is_null());
    unsafe {
        let s = CStr::from_ptr(cJSON_GetStringValue(cat)).to_string_lossy();
        assert_eq!(s, "🐱");
        cJSON_Delete(cat);
    }
}

#[test]
fn parse_invalid_backslash_and_truncated_escape_fail() {
    assert!(parse(r#""\e""#).is_null());
    assert!(parse("\"000000000000000000\\").is_null());
}

#[test]
fn parse_number_token_charset_digits_sign_exp_dot() {
    let item = parse("1.5e+10");
    assert!(!item.is_null());
    unsafe {
        assert!(cJSON_IsNumber(item) != 0);
        let n = cJSON_GetNumberValue(item);
        assert!((n - 1.5e10).abs() / 1.5e10 < 1e-12);
        cJSON_Delete(item);
    }
    // Trailing junk is OK (unless require_null_terminated).
    let item = parse("10e-10,");
    assert!(!item.is_null());
    unsafe { cJSON_Delete(item) };
}

#[test]
fn parse_skips_utf8_bom_only_at_offset_zero() {
    let with_bom = parse("\u{feff}{}");
    assert!(!with_bom.is_null());
    unsafe {
        assert!(cJSON_IsObject(with_bom) != 0);
        cJSON_Delete(with_bom);
    }
    assert!(
        parse(" \u{feff}{}").is_null(),
        "BOM not at offset 0 must not be skipped (whitespace skip runs after BOM)"
    );
}

#[test]
fn print_unformatted_roundtrips_literals() {
    for input in ["null", "true", "false", "1.5", "\"hello\"", "[]", "{}"] {
        let item = parse(input);
        assert!(!item.is_null(), "parse {input}");
        unsafe {
            let printed = take_c_string(cJSON_PrintUnformatted(item));
            assert_eq!(printed, input);
            cJSON_Delete(item);
        }
    }
}

#[test]
fn minify_strips_line_and_block_comments() {
    let mut line = CString::new("{// this is {} \"some kind\" of [] comment /*, don't you see\n}")
        .unwrap()
        .into_bytes_with_nul();
    unsafe { cJSON_Minify(line.as_mut_ptr() as *mut c_char) };
    let line = CStr::from_bytes_until_nul(&line).unwrap();
    assert_eq!(line.to_str().unwrap(), "{}");

    let mut block = CString::new("{/* this is\n a /* multi\n //line \n {comment \"\\\" */}")
        .unwrap()
        .into_bytes_with_nul();
    unsafe { cJSON_Minify(block.as_mut_ptr() as *mut c_char) };
    let block = CStr::from_bytes_until_nul(&block).unwrap();
    assert_eq!(block.to_str().unwrap(), "{}");
}

#[test]
fn minify_preserves_string_contents_and_unclosed_block_comment_is_empty() {
    let original = r#""this is a string \" \t bla""#;
    let mut s = CString::new(original).unwrap().into_bytes_with_nul();
    unsafe { cJSON_Minify(s.as_mut_ptr() as *mut c_char) };
    let s = CStr::from_bytes_until_nul(&s).unwrap();
    assert_eq!(s.to_str().unwrap(), original);

    let mut unclosed = CString::new("/* bla").unwrap().into_bytes_with_nul();
    unsafe { cJSON_Minify(unclosed.as_mut_ptr() as *mut c_char) };
    let unclosed = CStr::from_bytes_until_nul(&unclosed).unwrap();
    assert_eq!(unclosed.to_str().unwrap(), "");
}
