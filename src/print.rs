#![allow(dead_code)]
//! Safe JSON printer matching cJSON 1.7.19 byte-for-byte formatting.

use crate::traits::{PrintBuf, TreeReader};
use crate::types::{
    type_mask, CJSON_ARRAY, CJSON_FALSE, CJSON_NULL, CJSON_NUMBER, CJSON_OBJECT, CJSON_RAW,
    CJSON_STRING, CJSON_TRUE,
};

/// Render `item` into `buf`. Returns false on allocation/nesting failure.
///
/// Formatted objects: `{\n`, tab indent per depth, `:\t` after keys, `,\n` between
/// members. Formatted arrays: comma+space, no extra newlines for a flat array.
/// Unformatted: no spaces. Invalid type bits fail. Raw prints `valuestring` as-is.
pub fn print_value<R, P>(reader: &R, item: R::Handle, buf: &mut P) -> bool
where
    R: TreeReader,
    P: PrintBuf,
{
    let _ = (
        reader,
        item,
        buf,
        CJSON_NULL,
        CJSON_FALSE,
        CJSON_TRUE,
        CJSON_NUMBER,
        CJSON_STRING,
        CJSON_RAW,
        CJSON_ARRAY,
        CJSON_OBJECT,
        type_mask,
    );
    false
}

/// Escape `input` as a JSON string (including surrounding quotes) into `buf`.
/// NULL/empty input must print `""`.
pub fn print_string_ptr<P: PrintBuf>(input: Option<&[u8]>, buf: &mut P) -> bool {
    let _ = (input, buf);
    false
}
