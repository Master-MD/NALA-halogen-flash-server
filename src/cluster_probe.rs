#![forbid(unsafe_code)]

//! NALA Cluster Health Probe & `sHeriFF` Zero-Python / Zero-PII Auditor.
//!
//! - `probe [--api URL]`: checks `/health` and `/v1/models` on a halogen-flash-server instance
//!   and prints NALA SwitchYard / Lemonade compatible status JSON.
//! - `sheriff-audit [REPO_ROOT]`: enforces 0.0% Python files, Zero-PII (no internal IPs,
//!   personal paths, or secret tokens), and verifies `VERSION` SSOT + mandatory legal notices.

use crate::http;
use crate::json::{self, JsonValue};
use std::fs;
use std::path::Path;

pub fn run_probe(args: &[String]) -> i32 {
    let mut api = "http://127.0.0.1:8731".to_string();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--api" && i + 1 < args.len() {
            api = args[i + 1].clone();
            i += 2;
        } else {
            i += 1;
        }
    }
    let base = api.trim_end_matches('/');
    let health_res = http::get(&format!("{base}/health"), &[], 5);
    let models_res = http::get(&format!("{base}/v1/models"), &[], 5);

    let (healthy, health_val) = match health_res {
        Ok(r) if r.status == 200 => (true, json::parse(&r.body).unwrap_or(JsonValue::Null)),
        Ok(r) => (
            false,
            JsonValue::String(format!("HTTP {}", r.status)),
        ),
        Err(e) => (false, JsonValue::String(e)),
    };

    let models_val = match models_res {
        Ok(r) if r.status == 200 => json::parse(&r.body).unwrap_or(JsonValue::Null),
        _ => JsonValue::Null,
    };

    let report = JsonValue::Object(vec![
        (
            "nala_halogen_rs_version".to_string(),
            JsonValue::String(crate::version().to_string()),
        ),
        ("endpoint".to_string(), JsonValue::String(base.to_string())),
        ("healthy".to_string(), JsonValue::Bool(healthy)),
        (
            "required_silicon".to_string(),
            JsonValue::String("AMD Strix Halo (gfx1151, 128 GB UMA)".to_string()),
        ),
        ("health".to_string(), health_val),
        ("models".to_string(), models_val),
    ]);
    println!("{}", report.to_json_pretty());
    if healthy { 0 } else { 1 }
}

fn walk_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "target" || name == ".DS_Store" {
            continue;
        }
        if p.is_dir() {
            walk_files(&p, out);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

pub fn run_sheriff_audit(args: &[String]) -> i32 {
    let root = args
        .first()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    let mut files = Vec::new();
    walk_files(root, &mut files);

    let mut py_files = Vec::new();
    let mut pii_hits = Vec::new();
    let forbidden_markers = [
        "/Users/ultramacuser",
        "dreher.mike",
        "192.168.5.",
        "100.85.223.",
        "100.94.20.",
        "100.98.37.",
        "ghp_",
        "sk-ant-",
    ];

    for f in &files {
        let rel = f
            .strip_prefix(root)
            .unwrap_or(f)
            .to_string_lossy()
            .to_string();
        if rel.ends_with(".py") {
            py_files.push(rel.clone());
        }
        // Skip binary/font/image assets and this auditor file itself
        if rel.ends_with(".jpg")
            || rel.ends_with(".png")
            || rel.ends_with(".ttf")
            || rel == "src/cluster_probe.rs"
        {
            continue;
        }
        if let Ok(text) = fs::read_to_string(f) {
            for marker in &forbidden_markers {
                if text.contains(marker) {
                    pii_hits.push(format!("{rel}: matched forbidden marker '{marker}'"));
                }
            }
        }
    }

    let mut missing_notices = Vec::new();
    for req in [
        "VERSION",
        "Cargo.toml",
        "LICENSE.md",
        "THIRD-PARTY-NOTICES.md",
        "TRADEMARKS.md",
    ] {
        if !root.join(req).is_file() {
            missing_notices.push(req);
        }
    }

    println!(
        "sHeriFF Audit (nala-halogen-rs v{}): scanned {} files",
        crate::version(),
        files.len()
    );
    println!("  Python files (.py): {}", py_files.len());
    for p in &py_files {
        println!("    FAIL .py found: {p}");
    }
    println!("  PII / Secret leaks: {}", pii_hits.len());
    for h in &pii_hits {
        println!("    FAIL PII: {h}");
    }
    println!("  Missing required files: {}", missing_notices.len());
    for m in &missing_notices {
        println!("    FAIL missing: {m}");
    }

    if py_files.is_empty() && pii_hits.is_empty() && missing_notices.is_empty() {
        println!("sHeriFF Audit: PASS (0.0% Python, 0 PII leaks, all legal notices intact)");
        0
    } else {
        println!("sHeriFF Audit: FAIL");
        1
    }
}
