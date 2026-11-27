# NALA-halogen-flash-server (Rumantsch)

> **Utensils 100% Rust Nativ Pur e Segir (Edition 2024, `#![forbid(unsafe_code)]`, 0 Crates Externas, 0.0% Python)**
> Fork da [`peonist-ai/halogen-flash-server`](https://github.com/peonist-ai/halogen-flash-server) cun sincronisaziun automatica mintga di e remplazzament cumplet da tut ils scripts Python.

## Pertge quest fork?
Il repository original cuntegneva 17.2% Python (`tools/*.py`) per benchmarks, controlla da documentaziun, redirects e rendering da la pagina web. Quest fork NALA eliminescha **tut las 7 datotecas Python** e las remplazza cun in sulet binari Rust nativ senza dependenzas (`nala-halogen-rs`).

Il container uffizial (`ghcr.io/peonist-ai/halogen-flash-server:0.21.0`) vegn duvra senza midadas tenor `LICENSE.md` e `TRADEMARKS.md`.

## Hardware necessari
Ils kernels dal motor funcziunan exclusivamain sin **AMD Strix Halo (`gfx1151`, p.ex. Ryzen AI Max+ 395 / Radeon 8060S cun 128 GB UMA)** sut Linux (`/dev/kfd`, `/dev/dri`).

## Cumenzar spert (0% Python)
```bash
./install.sh
nala-halogen-rs --help
nala-halogen-rs sheriff-audit .
nala-halogen-rs probe --api http://127.0.0.1:8731
nala-halogen-rs halogen-bench --api http://127.0.0.1:8731 -p 512,2048 -n 128
```

## Linguas
[Deutsch](README_DE.md) | [English](README_EN.md) | [Francais](README_FR.md) | [Italiano](README_IT.md) | [Rumantsch](README_RM.md)
