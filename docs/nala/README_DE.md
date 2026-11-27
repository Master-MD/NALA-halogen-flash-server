# NALA-halogen-flash-server (Deutsch)

> **100% Native Pure Safe Rust Tooling (Edition 2024, `#![forbid(unsafe_code)]`, 0 externe Crates, 0.0% Python)**
> Fork von [`peonist-ai/halogen-flash-server`](https://github.com/peonist-ai/halogen-flash-server) mit automatischem Upstream-Sync und vollstaendiger Abloesung aller Python-Skripte.

## Warum dieser Fork?
Das Upstream-Repository enthielt 17.2% Python (`tools/*.py`) fuer Benchmarks, Doku-Pruefung, Redirect-Generierung und Website-Rendering. In diesem NALA-Fork wurden **alle 7 Python-Dateien vollstaendig entfernt** und durch eine einzige, blitzschnelle, abhaengigkeitsfreie Rust-Binary (`nala-halogen-rs`) ersetzt.

Das offizielle Container-Image (`ghcr.io/peonist-ai/halogen-flash-server:0.21.0`) bleibt gemaess `LICENSE.md` und `TRADEMARKS.md` unveraendert.

## Hardware-Voraussetzung (Wichtig!)
Die Engine-Kernels sind exklusiv fuer **AMD Strix Halo (`gfx1151`, z.B. Ryzen AI Max+ 395 / Radeon 8060S mit 128 GB Unified Memory)** kompiliert.
- **Laeuft auf:** AMD Strix Halo (`gfx1151`) unter Linux (ROCm/KFD, UMA Carve-Out im BIOS auf Minimum).
- **Laeuft NICHT auf:** AMD Vega 20 (Radeon Pro VII), RDNA2/3 diskreten GPUs, NVIDIA CUDA GPUs oder Apple Silicon (fuer diese Plattformen bitte `llama.cpp`, `ninfer`, `Ollama` oder `MLX` nutzen).

## Installation & Nutzung (0% Python)
```bash
./install.sh
nala-halogen-rs --help
nala-halogen-rs check-docs .
nala-halogen-rs sheriff-audit .
nala-halogen-rs probe --api http://127.0.0.1:8731
nala-halogen-rs halogen-bench --api http://127.0.0.1:8731 -p 512,2048 -n 128
nala-halogen-rs bench-serving serial,mtp 256 low 1 0.0
```

## Automatischer Upstream-Sync
Der GitHub-Workflow `.github/workflows/nala-rust-ci-and-upstream-sync.yml` prueft taeglich `peonist-ai/halogen-flash-server`, uebernimmt neue Releases/Dokumentation und erzwingt ueber `nala-halogen-rs sheriff-audit .` dauerhaft **0.0% Python** und **0 PII-Leaks**.

## Weitere Sprachen
[Deutsch](README_DE.md) | [English](README_EN.md) | [Francais](README_FR.md) | [Italiano](README_IT.md) | [Rumantsch](README_RM.md)
