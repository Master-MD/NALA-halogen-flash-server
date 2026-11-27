# NALA-halogen-flash-server (Francais)

> **Outillage 100% Rust Natif Pur et Sur (Edition 2024, `#![forbid(unsafe_code)]`, 0 Crate Externe, 0.0% Python)**
> Fork de [`peonist-ai/halogen-flash-server`](https://github.com/peonist-ai/halogen-flash-server) avec synchronisation upstream quotidienne automatisee et remplacement complet de tout le code Python.

## Pourquoi ce fork ?
Le depot upstream contenait 17.2% de Python (`tools/*.py`) pour les benchmarks, la verification de la documentation, les redirections et le rendu du site. Ce fork NALA supprime **les 7 scripts Python** et les remplace par un binaire Rust natif sans dependances (`nala-halogen-rs`).

L'image conteneur officielle (`ghcr.io/peonist-ai/halogen-flash-server:0.21.0`) reste intacte conformement a `LICENSE.md` et `TRADEMARKS.md`.

## Materiel requis
Les noyaux GPU sont compiles exclusivement pour **AMD Strix Halo (`gfx1151`, ex. Ryzen AI Max+ 395 / Radeon 8060S, 128 Go UMA)** sous Linux (`/dev/kfd`, `/dev/dri`).

## Demarrage rapide (0% Python)
```bash
./install.sh
nala-halogen-rs --help
nala-halogen-rs sheriff-audit .
nala-halogen-rs probe --api http://127.0.0.1:8731
nala-halogen-rs halogen-bench --api http://127.0.0.1:8731 -p 512,2048 -n 128
```

## Langues
[Deutsch](README_DE.md) | [English](README_EN.md) | [Francais](README_FR.md) | [Italiano](README_IT.md) | [Rumantsch](README_RM.md)
