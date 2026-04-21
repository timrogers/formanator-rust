//! Helpers shared between the `forma_api.rs` and `cli.rs` integration test
//! binaries.
//!
//! Each integration test binary in `tests/` is compiled as its own crate, so
//! we include this file with `#[path = "common/mod.rs"] mod common;` rather
//! than relying on Rust's normal module resolution.

use std::path::PathBuf;

/// Read a file from `tests/fixtures/` by name, panicking with a helpful
/// message on I/O errors.
pub fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

/// Create a temporary file with a `.jpg` extension and a minimal valid-looking
/// JPEG byte sequence. Suitable for tests that need to attach a receipt to a
/// multipart Forma claim request without depending on real image data.
pub fn make_fake_receipt() -> tempfile::NamedTempFile {
    use std::io::Write;
    let mut f = tempfile::Builder::new()
        .suffix(".jpg")
        .tempfile()
        .expect("tempfile");
    // SOI + JFIF marker + EOI is enough to satisfy reqwest's multipart
    // streaming and to look approximately like a JPEG to anything that peeks.
    f.write_all(&[
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0xFF, 0xD9,
    ])
    .expect("write fake receipt");
    f
}
