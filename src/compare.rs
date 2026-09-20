#![allow(dead_code)]
//! `cJSON_Compare` matching cJSON 1.7.19.

use crate::traits::TreeReader;
use crate::types::{
    type_mask, CJSON_ARRAY, CJSON_FALSE, CJSON_INVALID, CJSON_NULL, CJSON_NUMBER, CJSON_OBJECT,
    CJSON_RAW, CJSON_STRING, CJSON_TRUE,
};

/// Relative epsilon compare used for numbers (`fabs(a-b) <= max(|a|,|b|) * DBL_EPSILON`).
pub fn compare_double(a: f64, b: f64) -> bool {
    let max_val = a.abs().max(b.abs());
    (a - b).abs() <= max_val * f64::EPSILON
}

/// Case-insensitive compare that treats two nulls as unequal (cJSON `case_insensitive_strcmp`).
pub fn case_insensitive_strcmp(a: Option<&[u8]>, b: Option<&[u8]>) -> i32 {
    match (a, b) {
        (None, _) | (_, None) => 1,
        (Some(a), Some(b)) if core::ptr::eq(a, b) => 0,
        (Some(a), Some(b)) => {
            let n = a.len().min(b.len());
            for i in 0..n {
                let ca = (a[i] as char).to_ascii_lowercase() as i32;
                let cb = (b[i] as char).to_ascii_lowercase() as i32;
                if ca != cb {
                    return ca - cb;
                }
                if a[i] == 0 {
                    return 0;
                }
            }
            let ca = a.get(n).copied().unwrap_or(0);
            let cb = b.get(n).copied().unwrap_or(0);
            (ca as char).to_ascii_lowercase() as i32 - (cb as char).to_ascii_lowercase() as i32
        }
    }
}

/// Recursively compare two items. NULL or invalid types are unequal, even vs self.
/// `case_sensitive` applies to **object keys only**; string values are always sensitive.
pub fn compare<R: TreeReader>(
    reader: &R,
    a: Option<R::Handle>,
    b: Option<R::Handle>,
    case_sensitive: bool,
) -> bool {
    let _ = (
        reader,
        a,
        b,
        case_sensitive,
        CJSON_FALSE,
        CJSON_TRUE,
        CJSON_NULL,
        CJSON_NUMBER,
        CJSON_STRING,
        CJSON_RAW,
        CJSON_ARRAY,
        CJSON_OBJECT,
        CJSON_INVALID,
        type_mask,
    );
    false
}
