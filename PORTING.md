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

## Tests

### Rust (`cargo test`)

```text
cargo test
```

Must pass on the current stubs. `src/parse.rs` unit tests cover `parse_hex4`, UTF-8 BOM skip (`offset + 4 < length`, only at offset 0), whitespace (`byte <= 32`, step back at end), and the number-token charset (`0-9`, `+`, `-`, `e`, `E`, `.`).

Golden tests that need a working parser live in `tests/abi_golden.rs` and are `#[ignore]` (string escapes, number tokens, BOM via `cJSON_Parse`, print round-trip). Minify goldens in that file are enabled. Un-ignore the rest with `cargo test -- --ignored` once parse lands.

### Differential print bytes vs C oracle

`tests/differential.rs` compares (in future) `cJSON_PrintUnformatted` bytes against the original C library. It is a no-op unless `CJSON_ORACLE` is set:

```text
cmake -S . -B build -DENABLE_RUST_CJSON=ON
cmake --build build --target cjson_oracle
CJSON_ORACLE=build/libcjson_oracle.a cargo test --test differential
```

### CMake Unity tests with the Rust drop-in

```text
cmake -S . -B build -DENABLE_RUST_CJSON=ON
cmake --build build
ctest --test-dir build --output-on-failure
```

`ENABLE_RUST_CJSON` defaults **ON** if `cargo` is found, else **OFF**. Public Unity tests (`parse_examples`, `parse_with_opts`, `compare_tests`, `cjson_add`, `readme_examples`, `minify_tests`, `print_value`, `misc_tests`) are compiled with `CJSON_PUBLIC_TESTS_ONLY` (include `cJSON.h` only, link `target/release/libcjson.so` or `.a`). Internal Unity tests that call `parse_number` / `print_string` / `ensure` / `global_hooks` are skipped — those statics are **not** exported from the Rust cdylib.

`print_value.c` uses public `cJSON_Parse` + `cJSON_PrintUnformatted` under `CJSON_PUBLIC_TESTS_ONLY`. `misc_tests.c` keeps public-API cases and rewrites BOM checks through `cJSON_Parse`.

### C-only build (must keep working)

```text
cmake -S . -B build -DENABLE_RUST_CJSON=OFF
cmake --build build
ctest --test-dir build --output-on-failure
```

This compiles `cJSON.c` as `libcjson` exactly as upstream. Utils stay off by default (`ENABLE_CJSON_UTILS=OFF`).

### Fuzzing

AFL is not rewritten. `fuzzing/cjson_read_fuzzer.c` can link the Rust staticlib (`target/release/libcjson.a`) later the same way other C targets do when `ENABLE_RUST_CJSON=ON`.
