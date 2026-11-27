# NALA-halogen-flash-server (English)

> **100% Native Pure Safe Rust Tooling (Edition 2024, `#![forbid(unsafe_code)]`, 0 External Crates, 0.0% Python)**
> Fork of [`peonist-ai/halogen-flash-server`](https://github.com/peonist-ai/halogen-flash-server) with daily automated upstream sync and complete replacement of all Python tooling.

## Why This Fork?
The upstream repository shipped 17.2% Python (`tools/*.py`) for benchmarking, documentation gating, redirect generation, and site rendering. This NALA fork removes **all 7 Python scripts** and replaces them with a single zero-dependency native Rust binary (`nala-halogen-rs`).

The official container image (`ghcr.io/peonist-ai/halogen-flash-server:0.21.0`) is used unmodified in full compliance with `LICENSE.md` and `TRADEMARKS.md`.

## Hardware Requirement
The compiled engine kernels exclusively target **AMD Strix Halo (`gfx1151`, e.g., Ryzen AI Max+ 395 / Radeon 8060S with 128 GB Unified Memory)**.
- **Supported:** AMD Strix Halo (`gfx1151`) on Linux with `/dev/kfd` and `/dev/dri` (BIOS UMA carve-out set to minimum).
- **Not supported:** Discrete AMD Vega 20 / RDNA GPUs, NVIDIA GPUs, or Apple Silicon.

## Quickstart (0% Python)
```bash
./install.sh
nala-halogen-rs --help
nala-halogen-rs check-docs .
nala-halogen-rs sheriff-audit .
nala-halogen-rs probe --api http://127.0.0.1:8731
nala-halogen-rs halogen-bench --api http://127.0.0.1:8731 -p 512,2048 -n 128
nala-halogen-rs bench-serving serial,mtp 256 low 1 0.0
```

## Automated Upstream Sync
`.github/workflows/nala-rust-ci-and-upstream-sync.yml` runs Multi-OS Rust CI (Linux, macOS, Windows) and syncs daily with `peonist-ai/halogen-flash-server` while enforcing **0.0% Python** via `nala-halogen-rs sheriff-audit .`.

## Languages
[Deutsch](README_DE.md) | [English](README_EN.md) | [Francais](README_FR.md) | [Italiano](README_IT.md) | [Rumantsch](README_RM.md)
