# NALA-halogen-flash-server (Italiano)

> **Strumenti 100% Rust Nativo Puro e Sicuro (Edition 2024, `#![forbid(unsafe_code)]`, 0 Crate Esterne, 0.0% Python)**
> Fork di [`peonist-ai/halogen-flash-server`](https://github.com/peonist-ai/halogen-flash-server) con sincronizzazione upstream giornaliera automatica e rimozione totale di Python.

## Perche questo fork?
Il repository upstream includeva il 17.2% di Python (`tools/*.py`) per benchmark, validazione documentazione, redirect e rendering del sito. Questo fork NALA elimina **tutti i 7 file Python** sostituendoli con un singolo binario Rust nativo senza dipendenze esterne (`nala-halogen-rs`).

L'immagine container ufficiale (`ghcr.io/peonist-ai/halogen-flash-server:0.21.0`) viene usata senza modifiche nel pieno rispetto di `LICENSE.md` e `TRADEMARKS.md`.

## Requisiti Hardware
I kernel del motore sono compilati esclusivamente per **AMD Strix Halo (`gfx1151`, es. Ryzen AI Max+ 395 / Radeon 8060S con 128 GB UMA)** su Linux (`/dev/kfd`, `/dev/dri`).

## Avvio rapido (0% Python)
```bash
./install.sh
nala-halogen-rs --help
nala-halogen-rs sheriff-audit .
nala-halogen-rs probe --api http://127.0.0.1:8731
nala-halogen-rs halogen-bench --api http://127.0.0.1:8731 -p 512,2048 -n 128
```

## Lingue
[Deutsch](README_DE.md) | [English](README_EN.md) | [Francais](README_FR.md) | [Italiano](README_IT.md) | [Rumantsch](README_RM.md)
