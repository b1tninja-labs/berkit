//! Helpers for the conformance-corpus runner (`corpus.rs`): test-only, std only.

#[allow(dead_code)]
// shared with robustness.rs and src/tlv/differential.rs; other targets use none of it
pub mod inputs;
pub mod json;
pub mod model;
pub mod sha256;
pub mod typed;
