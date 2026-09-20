//! libc `strtod` / `sprintf` / `localeconv` adapters so number parse/print match C.

use crate::traits::{NumberParser, NumberPrinter, ParsedNumber};
use libc::{c_char, c_double, c_int};
use std::mem::MaybeUninit;

pub fn get_decimal_point() -> u8 {
    #[cfg(feature = "locales")]
    unsafe {
        let lconv = libc::localeconv();
        if lconv.is_null() {
            return b'.';
        }
        let dp = (*lconv).decimal_point;
        if dp.is_null() {
            b'.'
        } else {
            *dp as u8
        }
    }
    #[cfg(not(feature = "locales"))]
    {
        b'.'
    }
}

pub struct LibcNumber;

impl NumberParser for LibcNumber {
    fn parse_number(&self, input: &[u8]) -> Option<ParsedNumber> {
        if input.is_empty() {
            return None;
        }
        // Copy token bytes, remap '.' to locale decimal, NUL-terminate, strtod.
        let mut tmp = Vec::new();
        if tmp.try_reserve_exact(input.len() + 1).is_err() {
            return None;
        }
        tmp.extend_from_slice(input);
        let decimal = get_decimal_point();
        if decimal != b'.' {
            for b in &mut tmp {
                if *b == b'.' {
                    *b = decimal;
                }
            }
        }
        tmp.push(0);
        let mut end: *mut c_char = std::ptr::null_mut();
        let number = unsafe { libc::strtod(tmp.as_ptr() as *const c_char, &mut end) };
        if end as usize == tmp.as_ptr() as usize {
            return None;
        }
        let consumed = (end as usize).saturating_sub(tmp.as_ptr() as usize);
        let valueint = saturate_int(number);
        Some(ParsedNumber {
            valuedouble: number,
            valueint,
            consumed,
        })
    }
}

impl NumberPrinter for LibcNumber {
    fn print_number(&self, valuedouble: f64, valueint: i32, buf: &mut [u8]) -> Option<usize> {
        if buf.len() < 26 {
            return None;
        }
        let mut number_buffer: [MaybeUninit<u8>; 26] = [MaybeUninit::uninit(); 26];
        let ptr = number_buffer.as_mut_ptr() as *mut c_char;
        let length: c_int = unsafe {
            if valuedouble.is_nan() || valuedouble.is_infinite() {
                libc::sprintf(ptr, b"null\0".as_ptr() as *const c_char)
            } else if valuedouble == valueint as c_double {
                libc::sprintf(ptr, b"%d\0".as_ptr() as *const c_char, valueint as c_int)
            } else {
                let mut test: c_double = 0.0;
                let len15 = libc::sprintf(ptr, b"%1.15g\0".as_ptr() as *const c_char, valuedouble);
                let recovered = libc::sscanf(
                    ptr,
                    b"%lg\0".as_ptr() as *const c_char,
                    &mut test as *mut c_double,
                );
                if recovered != 1 || !crate::compare::compare_double(test, valuedouble) {
                    libc::sprintf(ptr, b"%1.17g\0".as_ptr() as *const c_char, valuedouble)
                } else {
                    len15
                }
            }
        };
        if length < 0 || length as usize > 25 {
            return None;
        }
        let n = length as usize;
        let decimal = get_decimal_point();
        for i in 0..n {
            let b = unsafe { *(ptr as *const u8).add(i) };
            buf[i] = if b == decimal { b'.' } else { b };
        }
        if n < buf.len() {
            buf[n] = 0;
        }
        Some(n)
    }
}

pub fn saturate_int(number: f64) -> i32 {
    if number >= i32::MAX as f64 {
        i32::MAX
    } else if number <= i32::MIN as f64 {
        i32::MIN
    } else {
        number as i32
    }
}
