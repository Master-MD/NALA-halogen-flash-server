# CAVEMAN GUIDE (TL;DR)

No Python. Only Rust. Fast. Safe. Zero crates.

## 1. Build Rust Tool (Mac / Linux)
```bash
./install.sh
```
Windows:
```powershell
.\install.ps1
```

## 2. Run Audit (0% Python, 0 PII)
```bash
nala-halogen-rs sheriff-audit .
nala-halogen-rs check-docs .
```

## 3. Check Live Strix Halo Server
```bash
nala-halogen-rs probe --api http://127.0.0.1:8731
nala-halogen-rs halogen-bench --api http://127.0.0.1:8731 -p 512,2048 -n 128
```

## 4. Hardware Rule
- Strix Halo (`gfx1151`, 128 GB UMA) = YES, VERY FAST (~100 tok/s code, ~1.2s agent TTFT).
- Vega 20 / NVIDIA / Mac = NO (engine binary is `gfx1151` only).
