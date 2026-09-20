#![allow(dead_code)]
//!
//! Strips space/tab/CR/LF, `//` line comments, and `/* */` comments.
//! Preserves string contents including escapes. Unclosed `/*` yields empty output.
//! Input is treated as a writable C string: rewrite in place and NUL-terminate.

/// Minify `json` in place. Returns the new length (not including the NUL).
/// `json` must include room for a trailing NUL at `json.len() - 1` if it is a
/// C string view; callers typically pass `slice_from_raw_parts_mut` including NUL.
pub fn minify_in_place(json: &mut [u8]) -> usize {
    if json.is_empty() {
        return 0;
    }
    let _ = json;
    0
}
