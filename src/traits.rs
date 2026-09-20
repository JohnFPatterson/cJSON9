#![allow(dead_code)]
//! Safe interfaces between the algorithm core and the unsafe FFI shim.
//!
//! Parser, printer, minify, and compare never dereference raw pointers; they
//! talk to the shim through these traits. Allocation failures are `None`/`false`,
//! never panics or aborting `Vec` growth.

pub const NESTING_LIMIT: usize = crate::types::CJSON_NESTING_LIMIT;
pub const CIRCULAR_LIMIT: usize = crate::types::CJSON_CIRCULAR_LIMIT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Byte offset into the input where parsing failed (cJSON `buffer.offset`).
    pub offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ParseOptions {
    pub require_null_terminated: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ParsedNumber {
    pub valuedouble: f64,
    pub valueint: i32,
    /// How many bytes of the token `strtod` consumed after locale remapping.
    pub consumed: usize,
}

pub trait NumberParser {
    fn parse_number(&self, input: &[u8]) -> Option<ParsedNumber>;
}

pub trait NumberPrinter {
    /// Write a number into `buf` (callers provide at least 26 bytes).
    /// Returns the number of bytes written, not including a trailing NUL.
    fn print_number(&self, valuedouble: f64, valueint: i32, buf: &mut [u8]) -> Option<usize>;
}

/// Builds a cJSON tree without the safe core touching raw pointers.
pub trait TreeBuilder {
    type Handle: Copy + Eq;

    fn new_item(&mut self) -> Option<Self::Handle>;
    fn set_null(&mut self, item: Self::Handle);
    fn set_bool(&mut self, item: Self::Handle, value: bool);
    fn set_number(&mut self, item: Self::Handle, valuedouble: f64, valueint: i32);
    fn set_array(&mut self, item: Self::Handle);
    fn set_object(&mut self, item: Self::Handle);

    /// Allocate `cap + 1` bytes, invoke `fill` with the writable prefix of length `cap`,
    /// NUL-terminate at the returned length, store as `valuestring`, type = String.
    fn set_string<F>(&mut self, item: Self::Handle, cap: usize, fill: F) -> bool
    where
        F: FnOnce(&mut [u8]) -> Option<usize>;

    /// After parsing an object key with `set_string`, move `valuestring` to `string`.
    fn move_valuestring_to_key(&mut self, item: Self::Handle);

    fn append_child(&mut self, parent: Self::Handle, child: Self::Handle) -> bool;
    fn delete(&mut self, item: Self::Handle);
    fn number_parser(&self) -> &dyn NumberParser;
}

pub trait TreeReader {
    type Handle: Copy;

    fn raw_type(&self, item: Self::Handle) -> i32;
    fn valuedouble(&self, item: Self::Handle) -> f64;
    fn valueint(&self, item: Self::Handle) -> i32;
    /// Bytes of `valuestring` without the trailing NUL, if present.
    fn valuestring(&self, item: Self::Handle) -> Option<&[u8]>;
    fn key(&self, item: Self::Handle) -> Option<&[u8]>;
    fn child(&self, item: Self::Handle) -> Option<Self::Handle>;
    fn next(&self, item: Self::Handle) -> Option<Self::Handle>;
}

pub trait PrintBuf {
    fn formatted(&self) -> bool;
    fn depth(&self) -> usize;
    fn inc_depth(&mut self);
    fn dec_depth(&mut self);
    fn write(&mut self, bytes: &[u8]) -> bool;
    fn number_printer(&self) -> &dyn NumberPrinter;
}
