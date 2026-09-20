//! Drop-in Rust implementation of cJSON 1.7.19.
//!
//! `unsafe` is denied at the crate root and allowed only in [`shim`].

#![deny(unsafe_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod compare;
mod minify;
mod parse;
mod print;
mod traits;
mod types;

#[allow(unsafe_code)]
mod shim;

pub use types::{
    CJSON_ARRAY, CJSON_CIRCULAR_LIMIT, CJSON_FALSE, CJSON_INVALID, CJSON_IS_REFERENCE,
    CJSON_NESTING_LIMIT, CJSON_NULL, CJSON_NUMBER, CJSON_OBJECT, CJSON_RAW, CJSON_STRING,
    CJSON_STRING_IS_CONST, CJSON_TRUE, CJSON_VERSION_MAJOR, CJSON_VERSION_MINOR,
    CJSON_VERSION_PATCH,
};
