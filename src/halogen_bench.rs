#![forbid(unsafe_code)]

//! Pure Safe Rust replacement for `tools/halogen-bench.py`.
//!
//! `llama-bench`-shaped `pp`/`tg` sweep over `halogen-flash-server`'s OpenAI-compatible HTTP API.

use crate::bench_serving::load_eval_prompts;
use crate::http;
use crate::json::{self, JsonValue};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::time::Instant;

const FILLER: &str = "The unified memory architecture changes how inference engines schedule work across the accelerator and the host processor. ";

fn lead_line(i: usize) -> String {
    let words: Vec<&str> = FILLER.split_whitespace().collect();
    let k = 1 + (i % (words.len() - 1));
    format!("{} ", words[k..].join(" "))
}

fn mean_and_stdev(xs: &[f64]) -> (f64, f64) {
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let m = xs.iter().sum::<f64>() / xs.len() as f64;
    if xs.len() < 2 {
        return (m, 0.0);
    }
    let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() as f64 - 1.0);
    (m, var.sqrt())
}

fn ask(
    api: &str,
    auth: Option<&str>,
    prompt: &str,
    max_tokens: u64,
    drafter: Option<&str>,
    effort: &str,
) -> Result<(JsonValue, f64), String> {
    let mut fields = vec![
        (
            "messages".to_string(),
            JsonValue::Array(vec![JsonValue::Object(vec![
                ("role".to_string(), JsonValue::String("user".to_string())),
                ("content".to_string(), JsonValue::String(prompt.to_string())),
            ])]),
        ),
        ("max_tokens".to_string(), JsonValue::Number(max_tokens as f64)),
        ("enable_thinking".to_string(), JsonValue::Bool(false)),
        (
            "reasoning_effort".to_string(),
            JsonValue::String(effort.to_string()),
        ),
        ("temperature".to_string(), JsonValue::Number(0.0)),
    ];
    if let Some(dr) = drafter {
        fields.push(("drafter".to_string(), JsonValue::String(dr.to_string())));
    }
    let body = JsonValue::Object(fields).to_json_string();
    let mut hdrs = Vec::new();
    if let Some(a) = auth {
        hdrs.push(("Authorization", a));
    }
    let url = format!("{}/v1/chat/completions", api.trim_end_matches('/'));
    let t0 = Instant::now();
    let resp = http::post_json(&url, &body, &hdrs, 1800)?;
    let wall = t0.elapsed().as_secs_f64();
    if resp.status != 200 {
        return Err(format!("HTTP {}: {}", resp.status, resp.body));
    }
    let parsed = json::parse(&resp.body)?;
    Ok((parsed, wall))
}

struct PromptCalib {
    overhead: Option<f64>,
    per_unit: f64,
    leads: HashMap<String, f64>,
}

impl PromptCalib {
    fn new() -> Self {
        Self {
            overhead: None,
            per_unit: 20.0,
            leads: HashMap::new(),
        }
    }

    fn build_prompt(
        &mut self,
        api: &str,
        auth: Option<&str>,
        target_tokens: u64,
        effort: &str,
        lead: &str,
    ) -> Result<String, String> {
        if self.overhead.is_none() {
            let (d1, _) = ask(api, auth, "x", 1, None, effort)?;
            let ov = d1
                .get("usage")
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            self.overhead = Some(ov);
            let p20 = format!("x{}", FILLER.repeat(20));
            let (d2, _) = ask(api, auth, &p20, 1, None, effort)?;
            let pt20 = d2
                .get("usage")
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|v| v.as_f64())
                .unwrap_or(ov + 400.0);
            self.per_unit = ((pt20 - ov) / 20.0).max(1e-6);
        }
        let overhead = self.overhead.unwrap_or(0.0);
        let need = (target_tokens as f64 - overhead).max(0.0);
        let mut k = (need / self.per_unit).round().max(1.0) as usize;
        if lead != "x" {
            let lead_tok = if let Some(&v) = self.leads.get(lead) {
                v
            } else {
                let (dl, _) = ask(api, auth, &format!("{lead}{FILLER}"), 1, None, effort)?;
                let pt = dl
                    .get("usage")
                    .and_then(|u| u.get("prompt_tokens"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(overhead + self.per_unit);
                let val = pt - self.per_unit;
                self.leads.insert(lead.to_string(), val);
                val
            };
            let extra = (lead_tok - overhead).max(0.0);
            let sub = (extra / self.per_unit).ceil() as usize;
            k = k.saturating_sub(sub).max(1);
        }
        Ok(format!("{lead}{}", FILLER.repeat(k)))
    }
}

pub fn run(args: &[String]) -> i32 {
    let mut api = "http://127.0.0.1:8731".to_string();
    let mut p_arg = "512,2048".to_string();
    let mut n_arg = "128".to_string();
    let mut drafters_arg = String::new();
    let mut reps: usize = 3;
    let mut effort = "low".to_string();
    let mut tg_cases = String::new();
    let mut as_json = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--api" if i + 1 < args.len() => {
                api = args[i + 1].clone();
                i += 2;
            }
            "-p" if i + 1 < args.len() => {
                p_arg = args[i + 1].clone();
                i += 2;
            }
            "-n" if i + 1 < args.len() => {
                n_arg = args[i + 1].clone();
                i += 2;
            }
            "-d" | "--drafters" if i + 1 < args.len() => {
                drafters_arg = args[i + 1].clone();
                i += 2;
            }
            "-r" | "--reps" if i + 1 < args.len() => {
                reps = args[i + 1].parse().unwrap_or(3);
                i += 2;
            }
            "--effort" if i + 1 < args.len() => {
                effort = args[i + 1].clone();
                i += 2;
            }
            "--tg-cases" if i + 1 < args.len() => {
                tg_cases = args[i + 1].clone();
                i += 2;
            }
            "--json" => {
                as_json = true;
                i += 1;
            }
            _ => i += 1,
        }
    }

    let api_key = env::var("HALOGEN_API_KEY")
        .ok()
        .map(|k| k.split(',').next().unwrap_or("").trim().to_string())
        .filter(|k| !k.is_empty());
    let auth_header = api_key.as_ref().map(|k| format!("Bearer {k}"));

    let pps: Vec<u64> = p_arg
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let tgs: Vec<u64> = n_arg
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let drafters: Vec<Option<String>> = {
        let v: Vec<Option<String>> = drafters_arg
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| Some(s.to_string()))
            .collect();
        if v.is_empty() { vec![None] } else { v }
    };

    let health_url = format!("{}/health", api.trim_end_matches('/'));
    let health = match http::get(&health_url, &[], 10) {
        Ok(r) if r.status == 200 => match json::parse(&r.body) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("invalid JSON from {health_url}: {e}");
                return 2;
            }
        },
        Ok(r) => {
            eprintln!("cannot reach {health_url}: HTTP {}", r.status);
            return 2;
        }
        Err(e) => {
            eprintln!("cannot reach {health_url}: {e}");
            return 2;
        }
    };

    let model = health
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    let ctx = health
        .get("context")
        .map(|v| match v {
            JsonValue::String(s) => s.clone(),
            JsonValue::Number(n) => format!("{}", *n as u64),
            _ => "?".to_string(),
        })
        .unwrap_or_else(|| "?".to_string());
    let default_drafter = health
        .get("drafter_default")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    if !as_json {
        println!("halogen-bench: {model}, ctx {ctx}, reps {reps}, effort {effort}");
        println!("endpoint {api}  (HTTP: chat template + tokenizer + engine)\n");
    }

    let (mut tg_prompts, mut tg_src) = match load_eval_prompts(Path::new("tools/eval-prompts.json"))
    {
        Ok(v) if !v.is_empty() => (v, "eval-prompts.json".to_string()),
        _ => (
            vec![(
                "prose".to_string(),
                "Write a long detailed essay about distributed systems.".to_string(),
            )],
            "BUILT-IN PROSE ONLY, worst case, not representative".to_string(),
        ),
    };
    if !tg_cases.is_empty() {
        let want: Vec<&str> = tg_cases.split(',').map(|s| s.trim()).collect();
        let filtered: Vec<(String, String)> = tg_prompts
            .iter()
            .filter(|(k, _)| want.contains(&k.as_str()))
            .cloned()
            .collect();
        if !filtered.is_empty() {
            tg_prompts = filtered;
            tg_src = format!(
                "subset: {}",
                tg_prompts
                    .iter()
                    .map(|(k, _)| k.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
    }

    let mut calib = PromptCalib::new();
    let mut rows: Vec<(String, String, Vec<f64>, u64)> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    for dr in &drafters {
        let dname = dr.clone().unwrap_or_else(|| default_drafter.clone());
        for (ti, &target) in pps.iter().enumerate() {
            let mut samples = Vec::new();
            let mut actual = 0u64;
            for rep in 0..reps {
                let prompt = match calib.build_prompt(
                    &api,
                    auth_header.as_deref(),
                    target,
                    &effort,
                    &lead_line(ti * reps + rep),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("pp{target} failed: {e}");
                        return 1;
                    }
                };
                let (d, wall) =
                    match ask(&api, auth_header.as_deref(), &prompt, 1, dr.as_deref(), &effort) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("pp{target} request failed: {e}");
                            return 1;
                        }
                    };
                actual = d
                    .get("usage")
                    .and_then(|u| u.get("prompt_tokens"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(target);
                samples.push(actual as f64 / wall.max(1e-6));
                let hit = d
                    .get("timings")
                    .and_then(|t| t.get("cache_n"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if hit > 0 {
                    notes.push(format!(
                        "pp{target} request {}: {hit} of {actual} tokens came from the prompt cache, so that cell is not cold",
                        rep + 1
                    ));
                }
            }
            rows.push((format!("pp{target}"), dname.clone(), samples, actual));
        }

        for &target in &tgs {
            let mut samples = Vec::new();
            let mut got = 0u64;
            for (_, cprompt) in &tg_prompts {
                for _ in 0..reps {
                    let (d, wall) = match ask(
                        &api,
                        auth_header.as_deref(),
                        cprompt,
                        target,
                        dr.as_deref(),
                        &effort,
                    ) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("tg{target} request failed: {e}");
                            return 1;
                        }
                    };
                    got = d
                        .get("usage")
                        .and_then(|u| u.get("completion_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    samples.push(got as f64 / wall.max(1e-6));
                }
            }
            rows.push((format!("tg{target}"), dname.clone(), samples, got));
            notes.push(format!(
                "tg{target} over {} prompt shapes x {reps} reps ({tg_src})",
                tg_prompts.len()
            ));
        }
    }

    if as_json {
        let json_rows: Vec<JsonValue> = rows
            .iter()
            .map(|(t, dr, s, n)| {
                let (m, sd) = mean_and_stdev(s);
                JsonValue::Object(vec![
                    ("test".to_string(), JsonValue::String(t.clone())),
                    ("drafter".to_string(), JsonValue::String(dr.clone())),
                    ("tokens".to_string(), JsonValue::Number(*n as f64)),
                    ("mean".to_string(), JsonValue::Number(m)),
                    ("stdev".to_string(), JsonValue::Number(sd)),
                ])
            })
            .collect();
        let out = JsonValue::Object(vec![
            ("model".to_string(), JsonValue::String(model)),
            ("context".to_string(), JsonValue::String(ctx)),
            ("reps".to_string(), JsonValue::Number(reps as f64)),
            ("rows".to_string(), JsonValue::Array(json_rows)),
        ]);
        println!("{}", out.to_json_pretty());
        return 0;
    }

    println!("| model | drafter | test | tokens | t/s | min-max |");
    println!("| --- | --- | --- | ---: | ---: | ---: |");
    for (test, dname, s, n) in &rows {
        let (mean, sd) = mean_and_stdev(s);
        let mn = s.iter().copied().fold(f64::INFINITY, f64::min);
        let mx = s.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        println!("| {model} | {dname} | {test} | {n} | {mean:.2} ± {sd:.2} | {mn:.1}-{mx:.1} |");
    }
    println!("\npp = prefill (prompt processing), tg = token generation.");
    println!(
        "Both are END-TO-END over HTTP and include that request's prefill, so tg slightly understates pure decode rate."
    );
    let mut seen = std::collections::HashSet::new();
    for note in notes {
        if seen.insert(note.clone()) {
            println!("  note: {note}");
        }
    }
    0
}
