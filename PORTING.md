# Rust cJSON port — file ownership for parallel work

The crate at the repo root is a drop-in `cJSON.h` ABI (`cdylib` + `staticlib` named `cjson` → `libcjson.so`). Original `cJSON.c` stays as the differential oracle. `unsafe` is denied at the crate root and allowed only in `src/shim/`.

## Do not change

- `cJSON.h` (the ABI contract, including macros)
- `cJSON.c` / `cJSON_Utils.*` (oracle / Utils out of scope)
- Files assigned to another agent (see below)

## Shared signatures (already wired from the shim)

`src/shim/ffi.rs` already calls these. Replace the stub **bodies**, keep the signatures:

```rust
// src/parse.rs
pub fn parse<B: TreeBuilder>(
    builder: &mut B,
    input: &[u8],
    opts: ParseOptions,
) -> Result<(B::Handle, usize), ParseError>

pub fn parse_hex4(input: &[u8]) -> u32
pub fn skip_whitespace(input: &[u8], offset: &mut usize)
pub fn skip_utf8_bom(input: &[u8], offset: &mut usize) -> bool

// src/print.rs
pub fn print_value<R, P>(reader: &R, item: R::Handle, buf: &mut P) -> bool
where R: TreeReader, P: PrintBuf
pub fn print_string_ptr<P: PrintBuf>(input: Option<&[u8]>, buf: &mut P) -> bool

// src/minify.rs
pub fn minify_in_place(json: &mut [u8]) -> usize

// src/compare.rs
pub fn compare<R: TreeReader>(
    reader: &R,
    a: Option<R::Handle>,
    b: Option<R::Handle>,
    case_sensitive: bool,
) -> bool
```

Traits live in `src/traits.rs`. Number parse/print goes through `NumberParser` / `NumberPrinter` (libc `strtod`/`sprintf` in the shim). Do not use aborting `Vec`/`String` growth in the safe core.

## Quirks that tests will fail on if you "fix" them

- Trailing junk OK unless `require_null_terminated`
- Whitespace = any byte `<= 32`; if skip hits `length`, offset steps back one
- UTF-8 BOM only at offset 0, and only if `offset+4 < length`
- Duplicate keys kept; first-wins; default key match is case-insensitive
- Invalid `\u` fails (no U+FFFD). UTF-8 often passed through
- Minify strips `//` and `/* */`; parse does not
- NaN/Inf print as `null`; `valueint` saturates to INT_MAX/MIN
- Formatted objects: `{\n`, tabs, `:\t` after keys
- Formatted arrays: `, ` (comma+space); no extra newlines for flat arrays
- Nesting limit 1000 on parse and print; circular limit 10000 on Duplicate only
- Type is a bitfield; predicates mask `& 0xFF` except `IsBool`

Spec: `cJSON.c` and the Unity tests. Version string stays `1.7.19`.

## Agent file ownership

| Agent | May edit |
|---|---|
| parse | `src/parse.rs` only, plus tests inside that file |
| print | `src/print.rs` only, plus tests inside that file |
| minify-compare | `src/minify.rs`, `src/compare.rs` only |
| tests | `tests/common.h`, `tests/CMakeLists.txt`, `CMakeLists.txt`, `tests/rust_*.c` if needed, `src/parse.rs`/`print.rs` tests only if adding `#[cfg(test)]` modules without rewriting logic |

Do not expand `unsafe` outside `src/shim/`.
