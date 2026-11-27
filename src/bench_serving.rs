#![forbid(unsafe_code)]

//! Pure Safe Rust replacement for `tools/bench-serving.py`.
//!
//! Benchmarks drafters against a live `halogen-flash-server` `/v1/chat/completions` endpoint,
//! parses `serve_api:` ledger lines from log file or `podman logs`, and verifies FIPS 180-4
//! SHA-256 byte-identity across drafters in greedy mode.

use crate::http;
use crate::json::{self, JsonValue};
use crate::sha256;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const SEED0: u64 = 0x5EED_0000;

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub drafter: String,
    pub tokens: u64,
    pub secs: f64,
    pub tok_s: f64,
    pub rounds: u64,
    pub commit_per_round: f64,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub prefill_secs: f64,
}

/// Parse a `serve_api:` ledger line without external regex crates.
/// Example:
/// `serve_api: mtp 256 tok in 2.50s = 102.40 t/s | 64 rounds, commit 4.00/round | prompt 120 (80 cached, 66.7%), prefill 0.12s`
pub fn parse_ledger_line(line: &str) -> Option<LedgerEntry> {
    let idx = line.find("serve_api: ")?;
    let rest = &line[idx + "serve_api: ".len()..];
    let parts: Vec<&str> = rest.split('|').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return None;
    }

    // Part 0: "<drafter> <tok> tok in <secs>s = <t/s> t/s"
    let p0: Vec<&str> = parts[0].split_whitespace().collect();
    if p0.len() < 7 {
        return None;
    }
    let drafter = p0[0].to_string();
    let tokens = p0[1].parse::<u64>().ok()?;
    let secs = p0[4].trim_end_matches('s').parse::<f64>().ok()?;
    let tok_s = p0[6].parse::<f64>().ok()?;

    // Part 1: "<rounds> rounds, commit <cpr>/round"
    let p1: Vec<&str> = parts[1].split_whitespace().collect();
    if p1.len() < 4 {
        return None;
    }
    let rounds = p1[0].parse::<u64>().ok()?;
    let commit_per_round = p1[3]
        .trim_end_matches("/round")
        .parse::<f64>()
        .ok()?;

    // Part 2: "prompt <ptok> [(<cached> cached[, <pct>%])], prefill <pf>s"
    let p2 = parts[2];
    let after_prompt = p2.strip_prefix("prompt ")?.trim();
    let prompt_tokens = after_prompt
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse::<u64>()
        .ok()?;
    let cached_tokens = if let Some(paren_idx) = after_prompt.find('(') {
        let inside = &after_prompt[paren_idx + 1..];
        if inside.contains("cached") {
            inside
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };
    let pf_idx = after_prompt.find("prefill ")?;
    let pf_str = after_prompt[pf_idx + "prefill ".len()..]
        .split_whitespace()
        .next()?
        .trim_end_matches('s');
    let prefill_secs = pf_str.parse::<f64>().ok()?;

    Some(LedgerEntry {
        drafter,
        tokens,
        secs,
        tok_s,
        rounds,
        commit_per_round,
        prompt_tokens,
        cached_tokens,
        prefill_secs,
    })
}

fn read_api_output(api_log: &str, api_ctr: &str) -> String {
    if !api_log.is_empty() {
        let Ok(content) = fs::read_to_string(api_log) else {
            return String::new();
        };
        let mut ring: VecDeque<&str> = VecDeque::with_capacity(2000);
        for ln in content.lines() {
            if ring.len() == 2000 {
                ring.pop_front();
            }
            ring.push_back(ln);
        }
        return ring.into_iter().collect::<Vec<_>>().join("\n");
    }
    match Command::new("podman")
        .args(["logs", "--tail", "2000", api_ctr])
        .output()
    {
        Ok(out) => {
            let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&out.stderr));
            s
        }
        Err(_) => String::new(),
    }
}

fn parse_all_ledger(text: &str) -> Vec<LedgerEntry> {
    text.lines().filter_map(parse_ledger_line).collect()
}

fn fnv1a_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn load_eval_prompts(path: &Path) -> Result<Vec<(String, String)>, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let val = json::parse(&raw)?;
    let obj = val
        .as_object()
        .ok_or_else(|| format!("{}: expected JSON object", path.display()))?;
    let mut cases = Vec::new();
    for (k, v) in obj {
        if k.starts_with('_') {
            continue;
        }
        let prompt = if let Some(s) = v.as_str() {
            s.to_string()
        } else if let Some(s) = v.get("prompt").and_then(|x| x.as_str()) {
            s.to_string()
        } else {
            continue;
        };
        cases.push((k.clone(), prompt));
    }
    Ok(cases)
}

pub fn run(args: &[String]) -> i32 {
    let api = env::var("HALOGEN_API").unwrap_or_else(|_| "http://127.0.0.1:8731".to_string());
    let api_key = env::var("HALOGEN_API_KEY")
        .ok()
        .map(|k| k.split(',').next().unwrap_or("").trim().to_string())
        .filter(|k| !k.is_empty());
    let auth_header = api_key.as_ref().map(|k| format!("Bearer {k}"));

    let prompts_path = env::var("HALOGEN_PROMPTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("tools/eval-prompts.json"));
    let api_ctr = env::var("HALOGEN_API_CTR").unwrap_or_else(|_| "halogen-api".to_string());
    let api_log = env::var("HALOGEN_API_LOG").unwrap_or_default();

    let depth: usize = env::var("HALOGEN_BENCH_DEPTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let prefix_path = env::var("HALOGEN_BENCH_PREFIX").unwrap_or_default();
    let cpt: f64 = env::var("HALOGEN_BENCH_CPT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3.6);

    let drafters: Vec<String> = args
        .first()
        .map(|s| s.as_str())
        .unwrap_or("serial,mtp")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let max_tokens: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(256);
    let effort = args.get(2).map(|s| s.as_str()).unwrap_or("low");
    let reps: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let temp: f64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let top_p: f64 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.95);

    let cases = match load_eval_prompts(&prompts_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("bench-serving: {e}");
            return 1;
        }
    };

    let (prefix, prefix_sha) = if depth > 0 {
        if prefix_path.is_empty() {
            eprintln!("HALOGEN_BENCH_DEPTH needs HALOGEN_BENCH_PREFIX=<text file>.");
            return 1;
        }
        let Ok(doc) = fs::read_to_string(&prefix_path) else {
            eprintln!("cannot read HALOGEN_BENCH_PREFIX={prefix_path}");
            return 1;
        };
        let want = (depth as f64 * cpt) as usize;
        if doc.chars().count() < want {
            eprintln!(
                "{prefix_path} holds {} chars, need ~{want} for depth {depth} at {cpt:.1} chars/token",
                doc.chars().count()
            );
            return 1;
        }
        let sliced: String = doc.chars().take(want).collect();
        let sha = sha256::hex(sliced.as_bytes())[..12].to_string();
        (
            format!("Here is a reference document.\n\n<document>\n{sliced}\n</document>\n\n"),
            sha,
        )
    } else {
        (String::new(), String::new())
    };

    let base = parse_all_ledger(&read_api_output(&api_log, &api_ctr)).len();
    let mut order: Vec<(String, String, f64, u64)> = Vec::new();
    let mut hashes: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    let mode_str = if temp > 0.0 {
        format!(", SAMPLED temp={temp} top_p={top_p}")
    } else {
        ", greedy".to_string()
    };
    let depth_str = if depth > 0 {
        format!(
            "\n  depth ~{depth} tok from {prefix_path} sha {prefix_sha} @ {cpt} chars/tok (measured length in the prompt column)"
        )
    } else {
        String::new()
    };
    println!(
        "bench: {} cases x {} drafters x {} rep(s), max_tokens={}, effort={}{}{}",
        cases.len(),
        drafters.len(),
        reps,
        max_tokens,
        effort,
        mode_str,
        depth_str
    );

    let url = format!("{}/v1/chat/completions", api.trim_end_matches('/'));
    for rep in 0..reps {
        for (name, prompt) in &cases {
            for dr in &drafters {
                let seed = SEED0 + (rep as u64) * 1000 + (fnv1a_hash(name) % 997);
                let mut fields = vec![
                    (
                        "messages".to_string(),
                        JsonValue::Array(vec![JsonValue::Object(vec![
                            ("role".to_string(), JsonValue::String("user".to_string())),
                            (
                                "content".to_string(),
                                JsonValue::String(format!("{prefix}{prompt}")),
                            ),
                        ])]),
                    ),
                    ("max_tokens".to_string(), JsonValue::Number(max_tokens as f64)),
                    (
                        "reasoning_effort".to_string(),
                        JsonValue::String(effort.to_string()),
                    ),
                    ("drafter".to_string(), JsonValue::String(dr.clone())),
                ];
                if temp > 0.0 {
                    fields.push(("temperature".to_string(), JsonValue::Number(temp)));
                    fields.push(("top_p".to_string(), JsonValue::Number(top_p)));
                    fields.push(("seed".to_string(), JsonValue::Number(seed as f64)));
                } else {
                    fields.push(("temperature".to_string(), JsonValue::Number(0.0)));
                }
                let body = JsonValue::Object(fields).to_json_string();
                let mut hdrs = Vec::new();
                if let Some(ref auth) = auth_header {
                    hdrs.push(("Authorization", auth.as_str()));
                }

                let t0 = Instant::now();
                let resp = match http::post_json(&url, &body, &hdrs, 900) {
                    Ok(r) if r.status == 200 => r,
                    Ok(r) => {
                        eprintln!("HTTP {} for {name}/{dr}: {}", r.status, r.body);
                        return 1;
                    }
                    Err(e) => {
                        eprintln!("request failed for {name}/{dr}: {e}");
                        return 1;
                    }
                };
                let wall = t0.elapsed().as_secs_f64();
                let parsed = match json::parse(&resp.body) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("invalid JSON response for {name}/{dr}: {e}");
                        return 1;
                    }
                };
                let ntok = parsed
                    .get("usage")
                    .and_then(|u| u.get("completion_tokens"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let msg = parsed
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|a| a.first())
                    .and_then(|c| c.get("message"));
                let reasoning = msg
                    .and_then(|m| m.get("reasoning_content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let content = msg
                    .and_then(|m| m.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let combined = format!("{reasoning}{content}");
                let h = sha256::hex(combined.as_bytes())[..12].to_string();

                let entry = hashes.entry(name.clone()).or_default();
                let prev = entry.entry(dr.clone()).or_insert_with(|| h.clone());
                let flag = if *prev == h {
                    ""
                } else {
                    "  !! UNSTABLE ACROSS REPS"
                };
                println!("  r{rep} {name:<9} {dr:<8} {ntok:4} tok  {wall:6.2}s wall{flag}");
                order.push((name.clone(), dr.clone(), wall, ntok));
            }
        }
    }

    let all_now = parse_all_ledger(&read_api_output(&api_log, &api_ctr));
    let delta = all_now.len().saturating_sub(base);
    let stats = if delta > 0 && delta <= all_now.len() {
        &all_now[all_now.len() - delta..]
    } else {
        &[]
    };

    let mut per: HashMap<(String, String), Vec<(f64, f64, f64, f64, u64, u64)>> = HashMap::new();
    let mut wall_only: HashMap<(String, String), Vec<(f64, u64)>> = HashMap::new();
    let mut si = 0usize;
    for (name, dr, wall, ntok) in &order {
        if si < stats.len() && stats[si].drafter == *dr {
            let st = &stats[si];
            si += 1;
            per.entry((name.clone(), dr.clone())).or_default().push((
                st.tok_s,
                st.commit_per_round,
                st.prefill_secs,
                *wall,
                st.cached_tokens,
                st.prompt_tokens,
            ));
        } else {
            wall_only
                .entry((name.clone(), dr.clone()))
                .or_default()
                .push((*wall, *ntok));
        }
    }

    if si != stats.len() {
        eprintln!(
            "\nWARNING: {} ledger line(s) matched no request, other traffic hit the pod during the run.",
            stats.len() - si
        );
    }

    let nospec: BTreeSet<String> = wall_only.keys().map(|(_, d)| d.clone()).collect();
    if !nospec.is_empty() {
        let joined = nospec.into_iter().collect::<Vec<_>>().join(", ");
        println!(
            "note: {joined} emit no ledger line (non-speculative); scored on wall and marked *, not comparable to engine t/s."
        );
    }

    println!(
        "\n{:<10} {:<9} {:>7} {:>10} {:>8} {:>7}{}",
        "case",
        "drafter",
        "t/s",
        "commit/rd",
        "prefill",
        "wall",
        if depth > 0 { "  prompt" } else { "" }
    );
    let mut agg: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    for (name, _) in &cases {
        for dr in &drafters {
            let key = (name.clone(), dr.clone());
            if let Some(v) = per.get(&key) {
                let len = v.len() as f64;
                let ts = v.iter().map(|x| x.0).sum::<f64>() / len;
                let cm = v.iter().map(|x| x.1).sum::<f64>() / len;
                let pf = v.iter().map(|x| x.2).sum::<f64>() / len;
                let wl = v.iter().map(|x| x.3).sum::<f64>() / len;
                let cached = v.iter().map(|x| x.4 as f64).sum::<f64>() / len;
                let ptok = v.iter().map(|x| x.5 as f64).sum::<f64>() / len;
                let spread = if v.len() > 1 {
                    let mx = v.iter().map(|x| x.0).fold(f64::NEG_INFINITY, f64::max);
                    let mn = v.iter().map(|x| x.0).fold(f64::INFINITY, f64::min);
                    format!("  (±{:.2})", (mx - mn) / 2.0)
                } else {
                    String::new()
                };
                let ch = if cached > 0.0 {
                    format!("  [{cached:.0} cached]")
                } else {
                    String::new()
                };
                let pt = if depth > 0 {
                    format!(" {ptok:7.0}")
                } else {
                    String::new()
                };
                println!(
                    "{name:<10} {dr:<9} {ts:7.2} {cm:10.2} {pf:7.2}s {wl:6.2}s{pt}{spread}{ch}"
                );
                agg.entry(dr.clone()).or_default().push((ts, cm));
            } else if let Some(w) = wall_only.get(&key) {
                let len = w.len() as f64;
                let mw = w.iter().map(|x| x.0).sum::<f64>() / len;
                let mt = w.iter().map(|x| x.1 as f64).sum::<f64>() / len;
                println!(
                    "{name:<10} {dr:<9} {:7.2}* {:9} {:7} {mw:6.2}s",
                    mt / mw,
                    "",
                    ""
                );
            }
        }
    }

    println!("\n=== identity (sha256 of reasoning+content, per case) ===");
    if temp > 0.0 {
        println!("SKIPPED, sampled runs are not expected to be identical across drafters.");
        return 0;
    }
    let bad: Vec<String> = hashes
        .iter()
        .filter(|(_, m)| m.values().collect::<BTreeSet<_>>().len() != 1)
        .map(|(k, _)| k.clone())
        .collect();
    if bad.is_empty() {
        println!("PASS, every drafter byte-identical on every case");
    } else {
        println!("FAIL, drafters disagree on: {bad:?}");
    }

    println!("\n=== means over all cases ===");
    let ref_dr = drafters.iter().find(|d| agg.contains_key(*d)).cloned();
    let ref_mean = ref_dr
        .as_ref()
        .and_then(|d| agg.get(d))
        .map(|v| v.iter().map(|x| x.0).sum::<f64>() / v.len() as f64)
        .unwrap_or(0.0);
    for dr in &drafters {
        let Some(v) = agg.get(dr) else {
            println!("{dr:<9} no engine t/s (non-speculative; see the wall-scored * rows above)");
            continue;
        };
        let ts: Vec<f64> = v.iter().map(|x| x.0).collect();
        let mean = ts.iter().sum::<f64>() / ts.len() as f64;
        let mn = ts.iter().copied().fold(f64::INFINITY, f64::min);
        let mx = ts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let cm = v.iter().map(|x| x.1).sum::<f64>() / v.len() as f64;
        let delta = if ref_dr.as_deref() != Some(dr.as_str()) && ref_mean > 0.0 {
            format!(
                "   {:+.1}% vs {}",
                (mean / ref_mean - 1.0) * 100.0,
                ref_dr.as_deref().unwrap_or("")
            )
        } else {
            String::new()
        };
        println!(
            "{dr:<9} mean {mean:6.2} t/s   min {mn:5.2}  max {mx:5.2}   commit {cm:.2}/round{delta}"
        );
    }
    if bad.is_empty() { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ledger() {
        let ln = "2026-11-27 serve_api: mtp 256 tok in 2.50s = 102.40 t/s | 64 rounds, commit 4.00/round | prompt 120 (80 cached, 66.7%), prefill 0.12s";
        let e = parse_ledger_line(ln).expect("ledger parsed");
        assert_eq!(e.drafter, "mtp");
        assert_eq!(e.tokens, 256);
        assert!((e.tok_s - 102.40).abs() < 1e-6);
        assert_eq!(e.rounds, 64);
        assert_eq!(e.prompt_tokens, 120);
        assert_eq!(e.cached_tokens, 80);
    }
}
