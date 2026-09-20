//! Constants and layout-independent type flags matching `cJSON.h`.

use libc::c_int;

pub const CJSON_VERSION_MAJOR: u32 = 1;
pub const CJSON_VERSION_MINOR: u32 = 7;
pub const CJSON_VERSION_PATCH: u32 = 19;

pub const CJSON_INVALID: c_int = 0;
pub const CJSON_FALSE: c_int = 1 << 0;
pub const CJSON_TRUE: c_int = 1 << 1;
pub const CJSON_NULL: c_int = 1 << 2;
pub const CJSON_NUMBER: c_int = 1 << 3;
pub const CJSON_STRING: c_int = 1 << 4;
pub const CJSON_ARRAY: c_int = 1 << 5;
pub const CJSON_OBJECT: c_int = 1 << 6;
pub const CJSON_RAW: c_int = 1 << 7;

pub const CJSON_IS_REFERENCE: c_int = 256;
pub const CJSON_STRING_IS_CONST: c_int = 512;

pub const CJSON_NESTING_LIMIT: usize = 1000;
pub const CJSON_CIRCULAR_LIMIT: usize = 10000;

#[inline]
pub fn type_mask(type_: c_int) -> c_int {
    type_ & 0xFF
}
