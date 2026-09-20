//! Print buffer using cJSON `ensure` growth (double, INT_MAX cap, hook realloc).

use super::hooks::{allocate, deallocate, reallocate, GLOBAL_HOOKS};
use super::libc_num::LibcNumber;
use crate::traits::{NumberPrinter, PrintBuf};
use libc::c_int;

pub struct CPrintBuf {
    pub buffer: *mut u8,
    pub length: usize,
    pub offset: usize,
    pub depth: usize,
    pub noalloc: bool,
    pub format: bool,
    numbers: LibcNumber,
}

impl CPrintBuf {
    pub unsafe fn new(prebuffer: usize, format: bool, noalloc: bool, buffer: *mut u8) -> Option<Self> {
        let buf = if buffer.is_null() {
            let p = unsafe { allocate(prebuffer.max(1)) };
            if p.is_null() {
                return None;
            }
            unsafe { *p = 0 };
            p
        } else {
            buffer
        };
        Some(Self {
            buffer: buf,
            length: prebuffer,
            offset: 0,
            depth: 0,
            noalloc,
            format,
            numbers: LibcNumber,
        })
    }

    pub unsafe fn ensure(&mut self, needed: usize) -> *mut u8 {
        if self.buffer.is_null() {
            return std::ptr::null_mut();
        }
        if self.length > 0 && self.offset >= self.length {
            return std::ptr::null_mut();
        }
        if needed > c_int::MAX as usize {
            return std::ptr::null_mut();
        }
        let needed_total = needed + self.offset + 1;
        if needed_total <= self.length {
            return unsafe { self.buffer.add(self.offset) };
        }
        if self.noalloc {
            return std::ptr::null_mut();
        }
        let newsize = if needed_total > (c_int::MAX as usize) / 2 {
            if needed_total <= c_int::MAX as usize {
                c_int::MAX as usize
            } else {
                return std::ptr::null_mut();
            }
        } else {
            needed_total * 2
        };
        let newbuffer = {
            let has_realloc = unsafe { GLOBAL_HOOKS.reallocate };
            if has_realloc.is_some() {
                let p = unsafe { reallocate(self.buffer, newsize) };
                if p.is_null() {
                    unsafe { deallocate(self.buffer) };
                    self.length = 0;
                    self.buffer = std::ptr::null_mut();
                    return std::ptr::null_mut();
                }
                p
            } else {
                let p = unsafe { allocate(newsize) };
                if p.is_null() {
                    unsafe { deallocate(self.buffer) };
                    self.length = 0;
                    self.buffer = std::ptr::null_mut();
                    return std::ptr::null_mut();
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(self.buffer, p, self.offset + 1);
                    deallocate(self.buffer);
                }
                p
            }
        };
        self.length = newsize;
        self.buffer = newbuffer;
        unsafe { newbuffer.add(self.offset) }
    }

    pub fn take_buffer(&mut self) -> *mut u8 {
        let b = self.buffer;
        self.buffer = std::ptr::null_mut();
        b
    }

    pub unsafe fn free_buffer(&mut self) {
        if !self.buffer.is_null() && !self.noalloc {
            unsafe { deallocate(self.buffer) };
            self.buffer = std::ptr::null_mut();
        }
    }
}

impl PrintBuf for CPrintBuf {
    fn formatted(&self) -> bool {
        self.format
    }

    fn depth(&self) -> usize {
        self.depth
    }

    fn inc_depth(&mut self) {
        self.depth += 1;
    }

    fn dec_depth(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn write(&mut self, bytes: &[u8]) -> bool {
        let dest = unsafe { self.ensure(bytes.len() + 1) };
        if dest.is_null() {
            return false;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, bytes.len());
            *dest.add(bytes.len()) = 0;
        }
        self.offset += bytes.len();
        true
    }

    fn number_printer(&self) -> &dyn NumberPrinter {
        &self.numbers
    }
}
