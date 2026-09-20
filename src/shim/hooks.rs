//! Global malloc/free/realloc hooks matching `cJSON_InitHooks`.

use libc::{c_void, size_t};
use std::ptr;

pub type MallocFn = unsafe extern "C" fn(size_t) -> *mut c_void;
pub type FreeFn = unsafe extern "C" fn(*mut c_void);
pub type ReallocFn = unsafe extern "C" fn(*mut c_void, size_t) -> *mut c_void;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Hooks {
    pub malloc_fn: Option<MallocFn>,
    pub free_fn: Option<FreeFn>,
}

#[derive(Copy, Clone)]
pub struct InternalHooks {
    pub allocate: MallocFn,
    pub deallocate: FreeFn,
    pub reallocate: Option<ReallocFn>,
}

pub static mut GLOBAL_HOOKS: InternalHooks = InternalHooks {
    allocate: libc::malloc,
    deallocate: libc::free,
    reallocate: Some(libc::realloc),
};

pub unsafe fn allocate(size: usize) -> *mut u8 {
    let ptr = unsafe { (GLOBAL_HOOKS.allocate)(size as size_t) };
    ptr.cast()
}

pub unsafe fn deallocate(ptr: *mut u8) {
    if !ptr.is_null() {
        unsafe { (GLOBAL_HOOKS.deallocate)(ptr.cast()) };
    }
}

pub unsafe fn reallocate(ptr: *mut u8, size: usize) -> *mut u8 {
    match unsafe { GLOBAL_HOOKS.reallocate } {
        Some(realloc_fn) => unsafe { realloc_fn(ptr.cast(), size as size_t) }.cast(),
        None => ptr::null_mut(),
    }
}

fn fn_eq_malloc(f: MallocFn) -> bool {
    f as usize == libc::malloc as usize
}

fn fn_eq_free(f: FreeFn) -> bool {
    f as usize == libc::free as usize
}

pub unsafe fn init_hooks(hooks: *mut Hooks) {
    if hooks.is_null() {
        unsafe {
            GLOBAL_HOOKS.allocate = libc::malloc;
            GLOBAL_HOOKS.deallocate = libc::free;
            GLOBAL_HOOKS.reallocate = Some(libc::realloc);
        }
        return;
    }

    unsafe {
        GLOBAL_HOOKS.allocate = libc::malloc;
        GLOBAL_HOOKS.deallocate = libc::free;
        let h = &*hooks;
        if let Some(malloc_fn) = h.malloc_fn {
            GLOBAL_HOOKS.allocate = malloc_fn;
        }
        if let Some(free_fn) = h.free_fn {
            GLOBAL_HOOKS.deallocate = free_fn;
        }
        GLOBAL_HOOKS.reallocate = None;
        if fn_eq_malloc(GLOBAL_HOOKS.allocate) && fn_eq_free(GLOBAL_HOOKS.deallocate) {
            GLOBAL_HOOKS.reallocate = Some(libc::realloc);
        }
    }
}
