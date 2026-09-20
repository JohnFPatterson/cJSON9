//! Every `#[no_mangle]` C ABI export matching `cJSON.h`.

use super::builder::{CJsonBuilder, CJsonReader};
use super::cJSON;
use super::error::{fail_parse, reset_error, GetErrorPtr};
use super::hooks::{init_hooks, Hooks};
use super::node::{add_item_to_array, delete, malloc_c, free_c};
use super::printbuf::CPrintBuf;
use super::tree;
use crate::traits::ParseOptions;
use crate::types::*;
use libc::{c_char, c_double, c_int, c_void, size_t};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(default)
}

macro_rules! export {
    ($vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block) => {
        #[no_mangle]
        #[cfg(not(windows))]
        $vis unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? $body

        #[no_mangle]
        #[cfg(windows)]
        $vis unsafe extern "stdcall" fn $name($($arg: $ty),*) $(-> $ret)? $body
    };
}

export! {
    pub fn cJSON_Version() -> *const c_char {
        static VERSION: &[u8] = b"1.7.19\0";
        VERSION.as_ptr() as *const c_char
    }
}

export! {
    pub fn cJSON_InitHooks(hooks: *mut Hooks) {
        guard((), || unsafe { init_hooks(hooks) })
    }
}

export! {
    pub fn cJSON_GetErrorPtr() -> *const c_char {
        guard(ptr::null(), || unsafe { GetErrorPtr() })
    }
}

unsafe fn parse_with_length_opts(
    value: *const c_char,
    buffer_length: usize,
    return_parse_end: *mut *const c_char,
    require_null_terminated: bool,
) -> *mut cJSON {
    unsafe { reset_error() };
    if value.is_null() || buffer_length == 0 {
        return ptr::null_mut();
    }
    let input = unsafe { std::slice::from_raw_parts(value as *const u8, buffer_length) };
    let mut builder = CJsonBuilder::new();
    match crate::parse::parse(
        &mut builder,
        input,
        ParseOptions {
            require_null_terminated,
        },
    ) {
        Ok((root, end)) => {
            if !return_parse_end.is_null() {
                unsafe { *return_parse_end = value.add(end) };
            }
            root
        }
        Err(err) => {
            let pos = unsafe { fail_parse(value as *const u8, buffer_length, err.offset) };
            if !return_parse_end.is_null() {
                unsafe { *return_parse_end = value.add(pos) };
            }
            ptr::null_mut()
        }
    }
}

export! {
    pub fn cJSON_ParseWithLengthOpts(
        value: *const c_char,
        buffer_length: size_t,
        return_parse_end: *mut *const c_char,
        require_null_terminated: c_int,
    ) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe {
            parse_with_length_opts(
                value,
                buffer_length,
                return_parse_end,
                require_null_terminated != 0,
            )
        })
    }
}

export! {
    pub fn cJSON_ParseWithOpts(
        value: *const c_char,
        return_parse_end: *mut *const c_char,
        require_null_terminated: c_int,
    ) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe {
            if value.is_null() {
                return ptr::null_mut();
            }
            let buffer_length = libc::strlen(value) + 1;
            parse_with_length_opts(
                value,
                buffer_length,
                return_parse_end,
                require_null_terminated != 0,
            )
        })
    }
}

export! {
    pub fn cJSON_Parse(value: *const c_char) -> *mut cJSON {
        unsafe { cJSON_ParseWithOpts(value, ptr::null_mut(), 0) }
    }
}

export! {
    pub fn cJSON_ParseWithLength(value: *const c_char, buffer_length: size_t) -> *mut cJSON {
        unsafe { cJSON_ParseWithLengthOpts(value, buffer_length, ptr::null_mut(), 0) }
    }
}

unsafe fn print_item(item: *const cJSON, format: bool) -> *mut c_char {
    if item.is_null() {
        return ptr::null_mut();
    }
    let mut buf = match unsafe { CPrintBuf::new(256, format, false, ptr::null_mut()) } {
        Some(b) => b,
        None => return ptr::null_mut(),
    };
    let reader = CJsonReader;
    let ok = crate::print::print_value(&reader, item as *mut cJSON, &mut buf);
    if !ok {
        unsafe { buf.free_buffer() };
        return ptr::null_mut();
    }
    buf.take_buffer() as *mut c_char
}

export! {
    pub fn cJSON_Print(item: *const cJSON) -> *mut c_char {
        guard(ptr::null_mut(), || unsafe { print_item(item, true) })
    }
}

export! {
    pub fn cJSON_PrintUnformatted(item: *const cJSON) -> *mut c_char {
        guard(ptr::null_mut(), || unsafe { print_item(item, false) })
    }
}

export! {
    pub fn cJSON_PrintBuffered(item: *const cJSON, prebuffer: c_int, fmt: c_int) -> *mut c_char {
        guard(ptr::null_mut(), || unsafe {
            if prebuffer < 0 || item.is_null() {
                return ptr::null_mut();
            }
            let mut buf = match CPrintBuf::new(prebuffer as usize, fmt != 0, false, ptr::null_mut()) {
                Some(b) => b,
                None => return ptr::null_mut(),
            };
            let reader = CJsonReader;
            if !crate::print::print_value(&reader, item as *mut cJSON, &mut buf) {
                buf.free_buffer();
                return ptr::null_mut();
            }
            buf.take_buffer() as *mut c_char
        })
    }
}

export! {
    pub fn cJSON_PrintPreallocated(
        item: *mut cJSON,
        buffer: *mut c_char,
        length: c_int,
        format: c_int,
    ) -> c_int {
        guard(0, || unsafe {
            if length < 0 || buffer.is_null() {
                return 0;
            }
            let mut buf = match CPrintBuf::new(length as usize, format != 0, true, buffer as *mut u8)
            {
                Some(b) => b,
                None => return 0,
            };
            let reader = CJsonReader;
            i32::from(crate::print::print_value(&reader, item, &mut buf))
        })
    }
}

export! {
    pub fn cJSON_Delete(item: *mut cJSON) {
        guard((), || unsafe { delete(item) })
    }
}

export! {
    pub fn cJSON_GetArraySize(array: *const cJSON) -> c_int {
        guard(0, || unsafe { tree::get_array_size(array) })
    }
}

export! {
    pub fn cJSON_GetArrayItem(array: *const cJSON, index: c_int) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe {
            if index < 0 {
                ptr::null_mut()
            } else {
                tree::get_array_item(array, index as usize)
            }
        })
    }
}

export! {
    pub fn cJSON_GetObjectItem(object: *const cJSON, string: *const c_char) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe { tree::get_object_item(object, string, false) })
    }
}

export! {
    pub fn cJSON_GetObjectItemCaseSensitive(object: *const cJSON, string: *const c_char) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe { tree::get_object_item(object, string, true) })
    }
}

export! {
    pub fn cJSON_HasObjectItem(object: *const cJSON, string: *const c_char) -> c_int {
        guard(0, || unsafe {
            i32::from(!tree::get_object_item(object, string, false).is_null())
        })
    }
}

export! {
    pub fn cJSON_GetStringValue(item: *const cJSON) -> *mut c_char {
        guard(ptr::null_mut(), || unsafe {
            if item.is_null() || ((*item).type_ & 0xFF) != CJSON_STRING {
                ptr::null_mut()
            } else {
                (*item).valuestring
            }
        })
    }
}

export! {
    pub fn cJSON_GetNumberValue(item: *const cJSON) -> c_double {
        guard(f64::NAN, || unsafe {
            if item.is_null() || ((*item).type_ & 0xFF) != CJSON_NUMBER {
                f64::NAN
            } else {
                (*item).valuedouble
            }
        })
    }
}

macro_rules! export_is {
    ($name:ident, $kind:expr) => {
        export! {
            pub fn $name(item: *const cJSON) -> c_int {
                guard(0, || tree::is_type(item, 0, $kind))
            }
        }
    };
}

export_is!(cJSON_IsInvalid, CJSON_INVALID);
export_is!(cJSON_IsFalse, CJSON_FALSE);
export_is!(cJSON_IsTrue, CJSON_TRUE);
export_is!(cJSON_IsNull, CJSON_NULL);
export_is!(cJSON_IsNumber, CJSON_NUMBER);
export_is!(cJSON_IsString, CJSON_STRING);
export_is!(cJSON_IsArray, CJSON_ARRAY);
export_is!(cJSON_IsObject, CJSON_OBJECT);
export_is!(cJSON_IsRaw, CJSON_RAW);

export! {
    pub fn cJSON_IsBool(item: *const cJSON) -> c_int {
        guard(0, || tree::is_type(item, CJSON_TRUE | CJSON_FALSE, 0))
    }
}

export! { pub fn cJSON_CreateNull() -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_null() }) } }
export! { pub fn cJSON_CreateTrue() -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_true() }) } }
export! { pub fn cJSON_CreateFalse() -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_false() }) } }
export! { pub fn cJSON_CreateBool(boolean: c_int) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_bool(boolean) }) } }
export! { pub fn cJSON_CreateNumber(num: c_double) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_number(num) }) } }
export! { pub fn cJSON_CreateString(string: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_string(string) }) } }
export! { pub fn cJSON_CreateRaw(raw: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_raw(raw) }) } }
export! { pub fn cJSON_CreateArray() -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_array() }) } }
export! { pub fn cJSON_CreateObject() -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_object() }) } }
export! { pub fn cJSON_CreateStringReference(string: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_string_reference(string) }) } }
export! { pub fn cJSON_CreateObjectReference(child: *const cJSON) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_object_reference(child) }) } }
export! { pub fn cJSON_CreateArrayReference(child: *const cJSON) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_array_reference(child) }) } }
export! { pub fn cJSON_CreateIntArray(numbers: *const c_int, count: c_int) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_int_array(numbers, count) }) } }
export! { pub fn cJSON_CreateFloatArray(numbers: *const f32, count: c_int) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_float_array(numbers, count) }) } }
export! { pub fn cJSON_CreateDoubleArray(numbers: *const c_double, count: c_int) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_double_array(numbers, count) }) } }
export! { pub fn cJSON_CreateStringArray(strings: *const *const c_char, count: c_int) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::create_string_array(strings, count) }) } }

export! {
    pub fn cJSON_AddItemToArray(array: *mut cJSON, item: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(add_item_to_array(array, item)) })
    }
}
export! {
    pub fn cJSON_AddItemToObject(object: *mut cJSON, string: *const c_char, item: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(tree::add_item_to_object_public(object, string, item, false)) })
    }
}
export! {
    pub fn cJSON_AddItemToObjectCS(object: *mut cJSON, string: *const c_char, item: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(tree::add_item_to_object_public(object, string, item, true)) })
    }
}
export! {
    pub fn cJSON_AddItemReferenceToArray(array: *mut cJSON, item: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(tree::add_item_reference_to_array(array, item)) })
    }
}
export! {
    pub fn cJSON_AddItemReferenceToObject(object: *mut cJSON, string: *const c_char, item: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(tree::add_item_reference_to_object(object, string, item)) })
    }
}

export! { pub fn cJSON_AddNullToObject(object: *mut cJSON, name: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_null_to_object(object, name) }) } }
export! { pub fn cJSON_AddTrueToObject(object: *mut cJSON, name: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_true_to_object(object, name) }) } }
export! { pub fn cJSON_AddFalseToObject(object: *mut cJSON, name: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_false_to_object(object, name) }) } }
export! { pub fn cJSON_AddBoolToObject(object: *mut cJSON, name: *const c_char, boolean: c_int) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_bool_to_object(object, name, boolean) }) } }
export! { pub fn cJSON_AddNumberToObject(object: *mut cJSON, name: *const c_char, number: c_double) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_number_to_object(object, name, number) }) } }
export! { pub fn cJSON_AddStringToObject(object: *mut cJSON, name: *const c_char, string: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_string_to_object(object, name, string) }) } }
export! { pub fn cJSON_AddRawToObject(object: *mut cJSON, name: *const c_char, raw: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_raw_to_object(object, name, raw) }) } }
export! { pub fn cJSON_AddObjectToObject(object: *mut cJSON, name: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_object_to_object(object, name) }) } }
export! { pub fn cJSON_AddArrayToObject(object: *mut cJSON, name: *const c_char) -> *mut cJSON { guard(ptr::null_mut(), || unsafe { tree::add_array_to_object(object, name) }) } }

export! {
    pub fn cJSON_DetachItemViaPointer(parent: *mut cJSON, item: *mut cJSON) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe { tree::detach_item_via_pointer(parent, item) })
    }
}
export! {
    pub fn cJSON_DetachItemFromArray(array: *mut cJSON, which: c_int) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe {
            if which < 0 {
                ptr::null_mut()
            } else {
                tree::detach_item_via_pointer(array, tree::get_array_item(array, which as usize))
            }
        })
    }
}
export! {
    pub fn cJSON_DeleteItemFromArray(array: *mut cJSON, which: c_int) {
        guard((), || unsafe { delete(cJSON_DetachItemFromArray(array, which)) })
    }
}
export! {
    pub fn cJSON_DetachItemFromObject(object: *mut cJSON, string: *const c_char) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe {
            tree::detach_item_via_pointer(object, tree::get_object_item(object, string, false))
        })
    }
}
export! {
    pub fn cJSON_DetachItemFromObjectCaseSensitive(object: *mut cJSON, string: *const c_char) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe {
            tree::detach_item_via_pointer(object, tree::get_object_item(object, string, true))
        })
    }
}
export! {
    pub fn cJSON_DeleteItemFromObject(object: *mut cJSON, string: *const c_char) {
        guard((), || unsafe { delete(cJSON_DetachItemFromObject(object, string)) })
    }
}
export! {
    pub fn cJSON_DeleteItemFromObjectCaseSensitive(object: *mut cJSON, string: *const c_char) {
        guard((), || unsafe { delete(cJSON_DetachItemFromObjectCaseSensitive(object, string)) })
    }
}

export! {
    pub fn cJSON_InsertItemInArray(array: *mut cJSON, which: c_int, newitem: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(tree::insert_item_in_array(array, which, newitem)) })
    }
}
export! {
    pub fn cJSON_ReplaceItemViaPointer(parent: *mut cJSON, item: *mut cJSON, replacement: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(tree::replace_item_via_pointer(parent, item, replacement)) })
    }
}
export! {
    pub fn cJSON_ReplaceItemInArray(array: *mut cJSON, which: c_int, newitem: *mut cJSON) -> c_int {
        guard(0, || unsafe {
            if which < 0 {
                0
            } else {
                i32::from(tree::replace_item_via_pointer(
                    array,
                    tree::get_array_item(array, which as usize),
                    newitem,
                ))
            }
        })
    }
}
export! {
    pub fn cJSON_ReplaceItemInObject(object: *mut cJSON, string: *const c_char, newitem: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(tree::replace_item_in_object(object, string, newitem, false)) })
    }
}
export! {
    pub fn cJSON_ReplaceItemInObjectCaseSensitive(object: *mut cJSON, string: *const c_char, newitem: *mut cJSON) -> c_int {
        guard(0, || unsafe { i32::from(tree::replace_item_in_object(object, string, newitem, true)) })
    }
}

export! {
    pub fn cJSON_Duplicate(item: *const cJSON, recurse: c_int) -> *mut cJSON {
        guard(ptr::null_mut(), || unsafe { tree::duplicate_rec(item, 0, recurse != 0) })
    }
}

export! {
    pub fn cJSON_Compare(a: *const cJSON, b: *const cJSON, case_sensitive: c_int) -> c_int {
        guard(0, || {
            let reader = CJsonReader;
            i32::from(crate::compare::compare(
                &reader,
                if a.is_null() { None } else { Some(a as *mut cJSON) },
                if b.is_null() { None } else { Some(b as *mut cJSON) },
                case_sensitive != 0,
            ))
        })
    }
}

export! {
    pub fn cJSON_Minify(json: *mut c_char) {
        guard((), || unsafe {
            if json.is_null() {
                return;
            }
            let len = libc::strlen(json);
            let slice = std::slice::from_raw_parts_mut(json as *mut u8, len + 1);
            let new_len = crate::minify::minify_in_place(slice);
            if new_len < slice.len() {
                *json.add(new_len) = 0;
            }
        })
    }
}

export! {
    pub fn cJSON_SetNumberHelper(object: *mut cJSON, number: c_double) -> c_double {
        guard(f64::NAN, || unsafe { tree::set_number_helper(object, number) })
    }
}

export! {
    pub fn cJSON_SetValuestring(object: *mut cJSON, valuestring: *const c_char) -> *mut c_char {
        guard(ptr::null_mut(), || unsafe { tree::set_valuestring(object, valuestring) })
    }
}

export! {
    pub fn cJSON_malloc(size: size_t) -> *mut c_void {
        guard(ptr::null_mut(), || unsafe { malloc_c(size) })
    }
}

export! {
    pub fn cJSON_free(object: *mut c_void) {
        guard((), || unsafe { free_c(object) })
    }
}

