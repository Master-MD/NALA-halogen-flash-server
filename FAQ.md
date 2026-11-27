# FAQ (Frequently Asked Questions)

## 1. Is this repository 100% Python-free?
Yes. All 7 upstream Python scripts (`tools/*.py`, previously 17.2% of the repository) have been removed and replaced by `nala-halogen-rs`, a 100% Pure Safe Rust (Edition 2024, `#![forbid(unsafe_code)]`, 0 external crates) binary.

## 2. Does it modify the upstream halogen container image?
No. Per `LICENSE.md` Section 3.1 and `TRADEMARKS.md`, the compiled `gfx1151` engine container `ghcr.io/peonist-ai/halogen-flash-server:0.21.0` is pulled and run completely unmodified. This fork provides the Zero-Python native Rust host tooling, benchmarking, documentation/site gates, health probes, and automated upstream sync.

## 3. Which GPUs are supported?
Only **AMD Strix Halo (`gfx1151`, Ryzen AI Max+ 395 / Radeon 8060S, 128 GB UMA)** on Linux (`/dev/kfd`, `/dev/dri`, BIOS UMA carve-out set to minimum). It will **not** run on discrete AMD Vega 20 (Radeon Pro VII), RDNA2/3 discrete GPUs, NVIDIA GPUs, or Apple Silicon.

## 4. How does Upstream Auto-Sync stay Python-free?
`.github/workflows/nala-rust-ci-and-upstream-sync.yml` fetches `peonist-ai/halogen-flash-server` daily, merges documentation and release metadata, strips any upstream `tools/*.py`, runs `cargo test`, and gates every commit with `nala-halogen-rs sheriff-audit .` (0.0% Python, 0 PII leaks).

## 5. Can it integrate with Lemonade or NALA SwitchYard?
Yes. `halogen-flash-server` exposes standard OpenAI (`/v1/chat/completions`, `/v1/responses`, `/v1/models`) and Anthropic (`/v1/messages`) APIs on a single port (`8731`), plus `/health`. `nala-halogen-rs probe --api http://127.0.0.1:8731` returns structured JSON telemetry ready for SwitchYard, Lemonade, or LiteLLM health checks.
