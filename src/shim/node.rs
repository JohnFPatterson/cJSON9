//! Node allocation, deletion, strdup, and child-list splice.

use super::cJSON;
use super::hooks::{allocate, deallocate};
use crate::types::{CJSON_IS_REFERENCE, CJSON_STRING_IS_CONST};
use libc::{c_char, c_void};
use std::ptr;

pub unsafe fn new_item() -> *mut cJSON {
    let ptr = unsafe { allocate(std::mem::size_of::<cJSON>()) } as *mut cJSON;
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe { ptr.write(cJSON::zeroed()) };
    ptr
}

pub unsafe fn cjson_strdup(string: *const c_char) -> *mut c_char {
    if string.is_null() {
        return ptr::null_mut();
    }
    let len = unsafe { libc::strlen(string) } + 1;
    let copy = unsafe { allocate(len) } as *mut c_char;
    if copy.is_null() {
        return ptr::null_mut();
    }
    unsafe { ptr::copy_nonoverlapping(string, copy, len) };
    copy
}

pub unsafe fn delete(mut item: *mut cJSON) {
    while !item.is_null() {
        let next = unsafe { (*item).next };
        if unsafe { (*item).type_ } & CJSON_IS_REFERENCE == 0 && !unsafe { (*item).child }.is_null()
        {
            unsafe { delete((*item).child) };
        }
        if unsafe { (*item).type_ } & CJSON_IS_REFERENCE == 0
            && !unsafe { (*item).valuestring }.is_null()
        {
            unsafe { deallocate((*item).valuestring as *mut u8) };
            unsafe { (*item).valuestring = ptr::null_mut() };
        }
        if unsafe { (*item).type_ } & CJSON_STRING_IS_CONST == 0
            && !unsafe { (*item).string }.is_null()
        {
            unsafe { deallocate((*item).string as *mut u8) };
            unsafe { (*item).string = ptr::null_mut() };
        }
        unsafe { deallocate(item as *mut u8) };
        item = next;
    }
}

pub unsafe fn suffix_object(prev: *mut cJSON, item: *mut cJSON) {
    unsafe {
        (*prev).next = item;
        (*item).prev = prev;
    }
}

/// Append using `child->prev` as tail pointer (cJSON undocumented optimization).
pub unsafe fn add_item_to_array(array: *mut cJSON, item: *mut cJSON) -> bool {
    if item.is_null() || array.is_null() || array == item {
        return false;
    }
    let child = unsafe { (*array).child };
    if child.is_null() {
        unsafe {
            (*array).child = item;
            (*item).prev = item;
            (*item).next = ptr::null_mut();
        }
    } else if !unsafe { (*child).prev }.is_null() {
        unsafe {
            suffix_object((*child).prev, item);
            (*array).child.prev_as_tail(item);
        }
    }
    true
}

trait ChildTail {
    unsafe fn prev_as_tail(self, item: *mut cJSON);
}

impl ChildTail for *mut cJSON {
    unsafe fn prev_as_tail(self, item: *mut cJSON) {
        unsafe { (*self).prev = item };
    }
}

pub unsafe fn malloc_c(size: usize) -> *mut c_void {
    unsafe { allocate(size) }.cast()
}

pub unsafe fn free_c(object: *mut c_void) {
    unsafe { deallocate(object as *mut u8) };
}
