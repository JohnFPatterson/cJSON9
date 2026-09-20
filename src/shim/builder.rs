//! `TreeBuilder` / `TreeReader` over raw `cJSON*` nodes.

use super::cJSON;
use super::hooks::{allocate, deallocate};
use super::libc_num::LibcNumber;
use super::node::{add_item_to_array, delete, new_item};
use crate::traits::{NumberParser, TreeBuilder, TreeReader};
use crate::types::*;
use libc::c_char;
use std::ptr;

pub struct CJsonBuilder {
    numbers: LibcNumber,
}

impl CJsonBuilder {
    pub fn new() -> Self {
        Self {
            numbers: LibcNumber,
        }
    }
}

impl TreeBuilder for CJsonBuilder {
    type Handle = *mut cJSON;

    fn new_item(&mut self) -> Option<Self::Handle> {
        let p = unsafe { new_item() };
        if p.is_null() {
            None
        } else {
            Some(p)
        }
    }

    fn set_null(&mut self, item: Self::Handle) {
        unsafe { (*item).type_ = CJSON_NULL };
    }

    fn set_bool(&mut self, item: Self::Handle, value: bool) {
        unsafe {
            (*item).type_ = if value { CJSON_TRUE } else { CJSON_FALSE };
            if value {
                (*item).valueint = 1;
            }
        }
    }

    fn set_number(&mut self, item: Self::Handle, valuedouble: f64, valueint: i32) {
        unsafe {
            (*item).type_ = CJSON_NUMBER;
            (*item).valuedouble = valuedouble;
            (*item).valueint = valueint;
        }
    }

    fn set_array(&mut self, item: Self::Handle) {
        unsafe { (*item).type_ = CJSON_ARRAY };
    }

    fn set_object(&mut self, item: Self::Handle) {
        unsafe { (*item).type_ = CJSON_OBJECT };
    }

    fn set_string<F>(&mut self, item: Self::Handle, cap: usize, fill: F) -> bool
    where
        F: FnOnce(&mut [u8]) -> Option<usize>,
    {
        let buf = unsafe { allocate(cap + 1) };
        if buf.is_null() {
            return false;
        }
        let slice = unsafe { std::slice::from_raw_parts_mut(buf, cap) };
        match fill(slice) {
            Some(used) if used <= cap => {
                unsafe {
                    *buf.add(used) = 0;
                    (*item).type_ = CJSON_STRING;
                    (*item).valuestring = buf as *mut c_char;
                }
                true
            }
            _ => {
                unsafe { deallocate(buf) };
                false
            }
        }
    }

    fn move_valuestring_to_key(&mut self, item: Self::Handle) {
        unsafe {
            (*item).string = (*item).valuestring;
            (*item).valuestring = ptr::null_mut();
        }
    }

    fn append_child(&mut self, parent: Self::Handle, child: Self::Handle) -> bool {
        unsafe { add_item_to_array(parent, child) }
    }

    fn delete(&mut self, item: Self::Handle) {
        unsafe { delete(item) };
    }

    fn number_parser(&self) -> &dyn NumberParser {
        &self.numbers
    }
}

pub struct CJsonReader;

impl TreeReader for CJsonReader {
    type Handle = *mut cJSON;

    fn raw_type(&self, item: Self::Handle) -> i32 {
        if item.is_null() {
            0
        } else {
            unsafe { (*item).type_ }
        }
    }

    fn valuedouble(&self, item: Self::Handle) -> f64 {
        unsafe { (*item).valuedouble }
    }

    fn valueint(&self, item: Self::Handle) -> i32 {
        unsafe { (*item).valueint }
    }

    fn valuestring(&self, item: Self::Handle) -> Option<&[u8]> {
        unsafe { cstr_bytes((*item).valuestring) }
    }

    fn key(&self, item: Self::Handle) -> Option<&[u8]> {
        unsafe { cstr_bytes((*item).string) }
    }

    fn child(&self, item: Self::Handle) -> Option<Self::Handle> {
        let c = unsafe { (*item).child };
        if c.is_null() {
            None
        } else {
            Some(c)
        }
    }

    fn next(&self, item: Self::Handle) -> Option<Self::Handle> {
        let n = unsafe { (*item).next };
        if n.is_null() {
            None
        } else {
            Some(n)
        }
    }
}

unsafe fn cstr_bytes<'a>(s: *mut c_char) -> Option<&'a [u8]> {
    if s.is_null() {
        None
    } else {
        let len = libc::strlen(s);
        Some(std::slice::from_raw_parts(s as *const u8, len))
    }
}
