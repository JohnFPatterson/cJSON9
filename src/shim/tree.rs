//! Tree mutation and query helpers (Create/Add/Detach/Replace/Get/Is*).

use super::cJSON;
use super::hooks::{deallocate};
use super::libc_num::saturate_int;
use super::node::{add_item_to_array, cjson_strdup, delete, new_item, suffix_object};
use crate::types::*;
use libc::{c_char, c_double, c_int};
use std::ptr;

pub unsafe fn get_array_size(array: *const cJSON) -> c_int {
    if array.is_null() {
        return 0;
    }
    let mut child = unsafe { (*array).child };
    let mut size: usize = 0;
    while !child.is_null() {
        size += 1;
        child = unsafe { (*child).next };
    }
    size as c_int
}

pub unsafe fn get_array_item(array: *const cJSON, mut index: usize) -> *mut cJSON {
    if array.is_null() {
        return ptr::null_mut();
    }
    let mut current = unsafe { (*array).child };
    while !current.is_null() && index > 0 {
        index -= 1;
        current = unsafe { (*current).next };
    }
    current
}

unsafe fn case_insensitive_strcmp(a: *const c_char, b: *const c_char) -> c_int {
    if a.is_null() || b.is_null() {
        return 1;
    }
    if a == b {
        return 0;
    }
    unsafe {
        let mut s1 = a as *const u8;
        let mut s2 = b as *const u8;
        loop {
            let c1 = libc::tolower(*s1 as c_int);
            let c2 = libc::tolower(*s2 as c_int);
            if c1 != c2 {
                return c1 - c2;
            }
            if *s1 == 0 {
                return 0;
            }
            s1 = s1.add(1);
            s2 = s2.add(1);
        }
    }
}

pub unsafe fn get_object_item(
    object: *const cJSON,
    name: *const c_char,
    case_sensitive: bool,
) -> *mut cJSON {
    if object.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    let mut current = unsafe { (*object).child };
    if case_sensitive {
        while !current.is_null()
            && !unsafe { (*current).string }.is_null()
            && unsafe { libc::strcmp(name, (*current).string) } != 0
        {
            current = unsafe { (*current).next };
        }
    } else {
        while !current.is_null()
            && unsafe { case_insensitive_strcmp(name, (*current).string) } != 0
        {
            current = unsafe { (*current).next };
        }
    }
    if current.is_null() || unsafe { (*current).string }.is_null() {
        ptr::null_mut()
    } else {
        current
    }
}

unsafe fn create_reference(item: *const cJSON) -> *mut cJSON {
    if item.is_null() {
        return ptr::null_mut();
    }
    let reference = unsafe { new_item() };
    if reference.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        ptr::copy_nonoverlapping(item, reference, 1);
        (*reference).string = ptr::null_mut();
        (*reference).type_ |= CJSON_IS_REFERENCE;
        (*reference).next = ptr::null_mut();
        (*reference).prev = ptr::null_mut();
    }
    reference
}

unsafe fn add_item_to_object(
    object: *mut cJSON,
    string: *const c_char,
    item: *mut cJSON,
    constant_key: bool,
) -> bool {
    if object.is_null() || string.is_null() || item.is_null() || object == item {
        return false;
    }
    let (new_key, new_type) = if constant_key {
        (string as *mut c_char, unsafe { (*item).type_ } | CJSON_STRING_IS_CONST)
    } else {
        let key = unsafe { cjson_strdup(string) };
        if key.is_null() {
            return false;
        }
        (key, unsafe { (*item).type_ } & !CJSON_STRING_IS_CONST)
    };
    if unsafe { (*item).type_ } & CJSON_STRING_IS_CONST == 0 && !unsafe { (*item).string }.is_null()
    {
        unsafe { deallocate((*item).string as *mut u8) };
    }
    unsafe {
        (*item).string = new_key;
        (*item).type_ = new_type;
    }
    unsafe { add_item_to_array(object, item) }
}

pub unsafe fn add_item_to_object_public(
    object: *mut cJSON,
    string: *const c_char,
    item: *mut cJSON,
    constant_key: bool,
) -> bool {
    unsafe { add_item_to_object(object, string, item, constant_key) }
}

pub unsafe fn add_item_reference_to_array(array: *mut cJSON, item: *mut cJSON) -> bool {
    if array.is_null() {
        return false;
    }
    unsafe { add_item_to_array(array, create_reference(item)) }
}

pub unsafe fn add_item_reference_to_object(
    object: *mut cJSON,
    string: *const c_char,
    item: *mut cJSON,
) -> bool {
    if object.is_null() || string.is_null() {
        return false;
    }
    unsafe { add_item_to_object(object, string, create_reference(item), false) }
}

macro_rules! add_to_object {
    ($object:expr, $name:expr, $created:expr) => {{
        let item = $created;
        if unsafe { add_item_to_object($object, $name, item, false) } {
            item
        } else {
            unsafe { delete(item) };
            ptr::null_mut()
        }
    }};
}

pub unsafe fn add_null_to_object(object: *mut cJSON, name: *const c_char) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_null() })
}
pub unsafe fn add_true_to_object(object: *mut cJSON, name: *const c_char) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_true() })
}
pub unsafe fn add_false_to_object(object: *mut cJSON, name: *const c_char) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_false() })
}
pub unsafe fn add_bool_to_object(
    object: *mut cJSON,
    name: *const c_char,
    boolean: c_int,
) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_bool(boolean) })
}
pub unsafe fn add_number_to_object(
    object: *mut cJSON,
    name: *const c_char,
    number: c_double,
) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_number(number) })
}
pub unsafe fn add_string_to_object(
    object: *mut cJSON,
    name: *const c_char,
    string: *const c_char,
) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_string(string) })
}
pub unsafe fn add_raw_to_object(
    object: *mut cJSON,
    name: *const c_char,
    raw: *const c_char,
) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_raw(raw) })
}
pub unsafe fn add_object_to_object(object: *mut cJSON, name: *const c_char) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_object() })
}
pub unsafe fn add_array_to_object(object: *mut cJSON, name: *const c_char) -> *mut cJSON {
    add_to_object!(object, name, unsafe { create_array() })
}

pub unsafe fn detach_item_via_pointer(parent: *mut cJSON, item: *mut cJSON) -> *mut cJSON {
    if parent.is_null()
        || item.is_null()
        || (item != unsafe { (*parent).child } && unsafe { (*item).prev }.is_null())
    {
        return ptr::null_mut();
    }
    if item != unsafe { (*parent).child } {
        unsafe { (*(*item).prev).next = (*item).next };
    }
    if !unsafe { (*item).next }.is_null() {
        unsafe { (*(*item).next).prev = (*item).prev };
    }
    if item == unsafe { (*parent).child } {
        unsafe { (*parent).child = (*item).next };
    } else if unsafe { (*item).next }.is_null() {
        unsafe { (*(*parent).child).prev = (*item).prev };
    }
    unsafe {
        (*item).prev = ptr::null_mut();
        (*item).next = ptr::null_mut();
    }
    item
}

pub unsafe fn insert_item_in_array(array: *mut cJSON, which: c_int, newitem: *mut cJSON) -> bool {
    if which < 0 || newitem.is_null() {
        return false;
    }
    let after = unsafe { get_array_item(array, which as usize) };
    if after.is_null() {
        return unsafe { add_item_to_array(array, newitem) };
    }
    if after != unsafe { (*array).child } && unsafe { (*after).prev }.is_null() {
        return false;
    }
    unsafe {
        (*newitem).next = after;
        (*newitem).prev = (*after).prev;
        (*after).prev = newitem;
        if after == (*array).child {
            (*array).child = newitem;
        } else {
            (*(*newitem).prev).next = newitem;
        }
    }
    true
}

pub unsafe fn replace_item_via_pointer(
    parent: *mut cJSON,
    item: *mut cJSON,
    replacement: *mut cJSON,
) -> bool {
    if parent.is_null() || unsafe { (*parent).child }.is_null() || replacement.is_null() || item.is_null()
    {
        return false;
    }
    if replacement == item {
        return true;
    }
    unsafe {
        (*replacement).next = (*item).next;
        (*replacement).prev = (*item).prev;
        if !(*replacement).next.is_null() {
            (*(*replacement).next).prev = replacement;
        }
        if (*parent).child == item {
            if (*(*parent).child).prev == (*parent).child {
                (*replacement).prev = replacement;
            }
            (*parent).child = replacement;
        } else {
            if !(*replacement).prev.is_null() {
                (*(*replacement).prev).next = replacement;
            }
            if (*replacement).next.is_null() {
                (*(*parent).child).prev = replacement;
            }
        }
        (*item).next = ptr::null_mut();
        (*item).prev = ptr::null_mut();
        delete(item);
    }
    true
}

pub unsafe fn replace_item_in_object(
    object: *mut cJSON,
    string: *const c_char,
    replacement: *mut cJSON,
    case_sensitive: bool,
) -> bool {
    if replacement.is_null() || string.is_null() {
        return false;
    }
    if unsafe { (*replacement).type_ } & CJSON_STRING_IS_CONST == 0
        && !unsafe { (*replacement).string }.is_null()
    {
        unsafe { deallocate((*replacement).string as *mut u8) };
    }
    let key = unsafe { cjson_strdup(string) };
    if key.is_null() {
        return false;
    }
    unsafe {
        (*replacement).string = key;
        (*replacement).type_ &= !CJSON_STRING_IS_CONST;
    }
    let target = unsafe { get_object_item(object, string, case_sensitive) };
    unsafe { replace_item_via_pointer(object, target, replacement) }
}

pub unsafe fn create_null() -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe { (*item).type_ = CJSON_NULL };
    }
    item
}
pub unsafe fn create_true() -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe { (*item).type_ = CJSON_TRUE };
    }
    item
}
pub unsafe fn create_false() -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe { (*item).type_ = CJSON_FALSE };
    }
    item
}
pub unsafe fn create_bool(boolean: c_int) -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe { (*item).type_ = if boolean != 0 { CJSON_TRUE } else { CJSON_FALSE } };
    }
    item
}
pub unsafe fn create_number(num: c_double) -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe {
            (*item).type_ = CJSON_NUMBER;
            (*item).valuedouble = num;
            (*item).valueint = saturate_int(num);
        }
    }
    item
}
pub unsafe fn create_string(string: *const c_char) -> *mut cJSON {
    let item = unsafe { new_item() };
    if item.is_null() {
        return item;
    }
    unsafe {
        (*item).type_ = CJSON_STRING;
        (*item).valuestring = cjson_strdup(string);
        if (*item).valuestring.is_null() {
            delete(item);
            return ptr::null_mut();
        }
    }
    item
}
pub unsafe fn create_raw(raw: *const c_char) -> *mut cJSON {
    let item = unsafe { new_item() };
    if item.is_null() {
        return item;
    }
    unsafe {
        (*item).type_ = CJSON_RAW;
        (*item).valuestring = cjson_strdup(raw);
        if (*item).valuestring.is_null() {
            delete(item);
            return ptr::null_mut();
        }
    }
    item
}
pub unsafe fn create_array() -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe { (*item).type_ = CJSON_ARRAY };
    }
    item
}
pub unsafe fn create_object() -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe { (*item).type_ = CJSON_OBJECT };
    }
    item
}
pub unsafe fn create_string_reference(string: *const c_char) -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe {
            (*item).type_ = CJSON_STRING | CJSON_IS_REFERENCE;
            (*item).valuestring = string as *mut c_char;
        }
    }
    item
}
pub unsafe fn create_object_reference(child: *const cJSON) -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe {
            (*item).type_ = CJSON_OBJECT | CJSON_IS_REFERENCE;
            (*item).child = child as *mut cJSON;
        }
    }
    item
}
pub unsafe fn create_array_reference(child: *const cJSON) -> *mut cJSON {
    let item = unsafe { new_item() };
    if !item.is_null() {
        unsafe {
            (*item).type_ = CJSON_ARRAY | CJSON_IS_REFERENCE;
            (*item).child = child as *mut cJSON;
        }
    }
    item
}

unsafe fn create_number_array<T, F>(values: *const T, count: c_int, mut conv: F) -> *mut cJSON
where
    F: FnMut(&T) -> c_double,
{
    if count < 0 || values.is_null() {
        return ptr::null_mut();
    }
    let a = unsafe { create_array() };
    let mut p: *mut cJSON = ptr::null_mut();
    let mut n: *mut cJSON = ptr::null_mut();
    for i in 0..count as usize {
        if a.is_null() {
            break;
        }
        let val = unsafe { conv(&*values.add(i)) };
        n = unsafe { create_number(val) };
        if n.is_null() {
            unsafe { delete(a) };
            return ptr::null_mut();
        }
        if i == 0 {
            unsafe { (*a).child = n };
        } else {
            unsafe { suffix_object(p, n) };
        }
        p = n;
    }
    if !a.is_null() && !unsafe { (*a).child }.is_null() {
        unsafe { (*(*a).child).prev = n };
    }
    a
}

pub unsafe fn create_int_array(numbers: *const c_int, count: c_int) -> *mut cJSON {
    unsafe { create_number_array(numbers, count, |n| *n as c_double) }
}
pub unsafe fn create_float_array(numbers: *const f32, count: c_int) -> *mut cJSON {
    unsafe { create_number_array(numbers, count, |n| *n as c_double) }
}
pub unsafe fn create_double_array(numbers: *const c_double, count: c_int) -> *mut cJSON {
    unsafe { create_number_array(numbers, count, |n| *n) }
}
pub unsafe fn create_string_array(strings: *const *const c_char, count: c_int) -> *mut cJSON {
    if count < 0 || strings.is_null() {
        return ptr::null_mut();
    }
    let a = unsafe { create_array() };
    let mut p: *mut cJSON = ptr::null_mut();
    let mut n: *mut cJSON = ptr::null_mut();
    for i in 0..count as usize {
        if a.is_null() {
            break;
        }
        n = unsafe { create_string(*strings.add(i)) };
        if n.is_null() {
            unsafe { delete(a) };
            return ptr::null_mut();
        }
        if i == 0 {
            unsafe { (*a).child = n };
        } else {
            unsafe { suffix_object(p, n) };
        }
        p = n;
    }
    if !a.is_null() && !unsafe { (*a).child }.is_null() {
        unsafe { (*(*a).child).prev = n };
    }
    a
}

pub unsafe fn set_number_helper(object: *mut cJSON, number: c_double) -> c_double {
    if object.is_null() {
        return f64::NAN;
    }
    unsafe {
        (*object).valueint = saturate_int(number);
        (*object).valuedouble = number;
        number
    }
}

pub unsafe fn set_valuestring(object: *mut cJSON, valuestring: *const c_char) -> *mut c_char {
    if object.is_null()
        || unsafe { (*object).type_ } & CJSON_STRING == 0
        || unsafe { (*object).type_ } & CJSON_IS_REFERENCE != 0
    {
        return ptr::null_mut();
    }
    if unsafe { (*object).valuestring }.is_null() || valuestring.is_null() {
        return ptr::null_mut();
    }
    let v1_len = unsafe { libc::strlen(valuestring) };
    let v2_len = unsafe { libc::strlen((*object).valuestring) };
    if v1_len <= v2_len {
        let vs = unsafe { (*object).valuestring };
        let overlap = !((valuestring.add(v1_len) as usize) < (vs as usize)
            || (vs.add(v2_len) as usize) < (valuestring as usize));
        if overlap {
            return ptr::null_mut();
        }
        unsafe { libc::strcpy(vs, valuestring) };
        return vs;
    }
    let copy = unsafe { cjson_strdup(valuestring) };
    if copy.is_null() {
        return ptr::null_mut();
    }
    if !unsafe { (*object).valuestring }.is_null() {
        unsafe { deallocate((*object).valuestring as *mut u8) };
    }
    unsafe { (*object).valuestring = copy };
    copy
}

pub unsafe fn duplicate_rec(item: *const cJSON, depth: usize, recurse: bool) -> *mut cJSON {
    if item.is_null() {
        return ptr::null_mut();
    }
    let newitem = unsafe { new_item() };
    if newitem.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        (*newitem).type_ = (*item).type_ & !CJSON_IS_REFERENCE;
        (*newitem).valueint = (*item).valueint;
        (*newitem).valuedouble = (*item).valuedouble;
        if !(*item).valuestring.is_null() {
            (*newitem).valuestring = cjson_strdup((*item).valuestring);
            if (*newitem).valuestring.is_null() {
                delete(newitem);
                return ptr::null_mut();
            }
        }
        if !(*item).string.is_null() {
            (*newitem).string = if (*item).type_ & CJSON_STRING_IS_CONST != 0 {
                (*item).string
            } else {
                cjson_strdup((*item).string)
            };
            if (*newitem).string.is_null() {
                delete(newitem);
                return ptr::null_mut();
            }
        }
    }
    if !recurse {
        return newitem;
    }
    let mut child = unsafe { (*item).child };
    let mut next: *mut cJSON = ptr::null_mut();
    let mut newchild: *mut cJSON = ptr::null_mut();
    while !child.is_null() {
        if depth >= CJSON_CIRCULAR_LIMIT {
            unsafe { delete(newitem) };
            return ptr::null_mut();
        }
        newchild = unsafe { duplicate_rec(child, depth + 1, true) };
        if newchild.is_null() {
            unsafe { delete(newitem) };
            return ptr::null_mut();
        }
        if !next.is_null() {
            unsafe {
                (*next).next = newchild;
                (*newchild).prev = next;
            }
            next = newchild;
        } else {
            unsafe { (*newitem).child = newchild };
            next = newchild;
        }
        child = unsafe { (*child).next };
    }
    if !newitem.is_null() && !unsafe { (*newitem).child }.is_null() {
        unsafe { (*(*newitem).child).prev = newchild };
    }
    newitem
}

pub fn is_type(item: *const cJSON, mask: c_int, kind: c_int) -> c_int {
    if item.is_null() {
        return 0;
    }
    let t = unsafe { (*item).type_ };
    if mask == 0 {
        i32::from((t & 0xFF) == kind)
    } else {
        i32::from((t & mask) != 0)
    }
}
