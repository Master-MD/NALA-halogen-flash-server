#![forbid(unsafe_code)]

//! `nala-halogen-rs` — 100% Native Pure Safe Rust (Edition 2024, Zero External Crates, Zero-Python)
//! tooling suite for `Master-MD/NALA-halogen-flash-server` (fork of `peonist-ai/halogen-flash-server`).
//!
//! Replaces all 7 upstream Python scripts (`bench-serving.py`, `halogen-bench.py`, `check_docs.py`,
//! `docs_redirects.py`, `check_redirects.py`, `site_release.py`, `mkdocs_hooks.py`) with a single
//! zero-dependency native binary, plus a NALA cluster health probe and `sHeriFF` Zero-Python/Zero-PII auditor.

pub mod bench_serving;
pub mod check_docs;
pub mod cluster_probe;
pub mod halogen_bench;
pub mod http;
pub mod json;
pub mod redirects;
pub mod sha256;
pub mod site_release;

/// Single-Source-of-Truth version loaded from `VERSION` at compile time.
pub const VERSION: &str = include_str!("../VERSION");

/// Trimmed version string.
pub fn version() -> &'static str {
    VERSION.trim()
}
