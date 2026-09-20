//! Unsafe FFI shim: C layout, allocators, pointer graphs, libc numbers, exports.
//!
//! This is the only module allowed to use `unsafe`.

mod builder;
mod error;
mod ffi;
mod hooks;
mod libc_num;
mod node;
mod printbuf;
mod tree;

use hooks::Hooks;
use libc::{c_char, c_double, c_int};

/// Matches `typedef struct cJSON` in `cJSON.h` field-for-field. No extra fields.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct cJSON {
    pub next: *mut cJSON,
    pub prev: *mut cJSON,
    pub child: *mut cJSON,
    pub type_: c_int,
    pub valuestring: *mut c_char,
    pub valueint: c_int,
    pub valuedouble: c_double,
    pub string: *mut c_char,
}

impl cJSON {
    pub const fn zeroed() -> Self {
        Self {
            next: core::ptr::null_mut(),
            prev: core::ptr::null_mut(),
            child: core::ptr::null_mut(),
            type_: 0,
            valuestring: core::ptr::null_mut(),
            valueint: 0,
            valuedouble: 0.0,
            string: core::ptr::null_mut(),
        }
    }
}

#[allow(dead_code)]
pub type cJSON_bool = c_int;
#[allow(dead_code)]
pub type cJSON_Hooks = Hooks;
