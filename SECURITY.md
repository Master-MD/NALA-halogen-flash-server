# Security Policy

## Supply-Chain & Memory Safety (`nala-halogen-rs`)
- **Zero Unsafe Code:** `#![forbid(unsafe_code)]` is enforced across every Rust source file in `src/`.
- **Zero External Crates:** `[dependencies]` in `Cargo.toml` is empty (`std` only), eliminating third-party crate supply-chain risks.
- **Zero Python Runtime:** All 7 upstream Python tools have been replaced by native compiled Rust.
- **Automated `sHeriFF` Gate:** `nala-halogen-rs sheriff-audit .` verifies 0.0% Python files, 0 PII/secret leaks, and intact legal notices on every CI run and upstream sync.

## Reporting Vulnerabilities
- For vulnerabilities in the upstream `halogen-flash-server` engine or container image, report directly to the upstream maintainers at `peonist-ai/halogen-flash-server` (see `README.md`).
- For issues in the NALA Rust tooling (`src/`), open a private security advisory on `Master-MD/NALA-halogen-flash-server`.
