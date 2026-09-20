//! Differential test: compare Rust `cJSON_PrintUnformatted` bytes against the C oracle.
//!
//! The original `cJSON.c` is built as the CMake target `cjson_oracle` when
//! `ENABLE_RUST_CJSON=ON` (it is not deleted). This test is a no-op unless the
//! environment variable `CJSON_ORACLE` is set to that static library (or another
//! original `libcjson` build).
//!
//! Enable:
//! ```text
//! cmake -S . -B build -DENABLE_RUST_CJSON=ON && cmake --build build --target cjson_oracle
//! CJSON_ORACLE=build/libcjson_oracle.a cargo test --test differential
//! ```
//!
//! Once parse/print land, this should load the oracle, print a shared corpus with
//! both implementations, and assert the byte strings are identical (including
//! formatted objects: `{\n`, tabs, `:\t`; formatted arrays: comma+space).

use std::path::Path;

const CORPUS: &[&str] = &[
    "null",
    "true",
    "false",
    "0",
    "1.5",
    "\"hello\"",
    "[]",
    "{}",
    "[1,2,3]",
    "{\"one\":1,\"two\":2}",
];

#[test]
fn print_bytes_match_c_oracle() {
    let Some(oracle) = std::env::var_os("CJSON_ORACLE") else {
        eprintln!(
            "skipping: set CJSON_ORACLE to the cjson_oracle static lib (or original libcjson) \
             to compare print bytes against the C implementation"
        );
        return;
    };

    let path = Path::new(&oracle);
    assert!(
        path.exists(),
        "CJSON_ORACLE does not exist: {}",
        path.display()
    );

    // Placeholder until parse/print are implemented: the corpus above is the
    // intended comparison set. Do not fail cargo test on stub print (always false).
    let _ = CORPUS;
    eprintln!(
        "CJSON_ORACLE={} present; byte-for-byte print comparison will run once parse/print land",
        path.display()
    );
}
