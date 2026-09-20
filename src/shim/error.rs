//! Global parse-error cursor matching `cJSON_GetErrorPtr`.

use libc::c_char;
use std::ptr;

struct ParseErrorState {
    json: *const u8,
    position: usize,
}

static mut GLOBAL_ERROR: ParseErrorState = ParseErrorState {
    json: ptr::null(),
    position: 0,
};

pub unsafe fn reset_error() {
    unsafe {
        GLOBAL_ERROR.json = ptr::null();
        GLOBAL_ERROR.position = 0;
    }
}

pub unsafe fn set_error(json: *const u8, position: usize) {
    unsafe {
        GLOBAL_ERROR.json = json;
        GLOBAL_ERROR.position = position;
    }
}

pub unsafe fn fail_parse(json: *const u8, length: usize, offset: usize) -> usize {
    let position = if offset < length {
        offset
    } else if length > 0 {
        length - 1
    } else {
        0
    };
    unsafe { set_error(json, position) };
    position
}

pub unsafe fn GetErrorPtr() -> *const c_char {
    unsafe { GLOBAL_ERROR.json.add(GLOBAL_ERROR.position) }.cast()
}
