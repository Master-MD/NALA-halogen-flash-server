#![forbid(unsafe_code)]

//! Pure Safe Rust replacement for `tools/docs_redirects.py`, `tools/check_redirects.py`,
//! and `tools/mkdocs_hooks.py` (`inject-blurbs`).

use crate::http;
use crate::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

const UA: &str = "halogen-docs-redirect-check/1";
const OLD_DEFAULT: &str = "https://peonist-ai.github.io/halogen-flash-server/";

fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
}

pub fn locs_of(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = xml[pos..].find("<loc>") {
        let start = pos + rel + 5;
        if let Some(end_rel) = xml[start..].find("</loc>") {
            let val = xml[start..start + end_rel].trim();
            out.push(html_unescape(val));
            pos = start + end_rel + 6;
        } else {
            break;
        }
    }
    out
}

fn common_base(locs: &[String]) -> String {
    let mut root = locs
        .iter()
        .min_by_key(|s| s.len())
        .cloned()
        .unwrap_or_default();
    if !root.ends_with('/') {
        root.push('/');
    }
    root
}

pub fn run_docs_redirects(args: &[String]) -> i32 {
    let mut site: Option<String> = None;
    let mut out: Option<String> = None;
    let mut old_prefix = "/halogen-flash-server/".to_string();

    let mut i = 0;
    while i < args.len() {
        if args[i] == "--old-prefix" && i + 1 < args.len() {
            old_prefix = args[i + 1].clone();
            i += 2;
        } else if site.is_none() {
            site = Some(args[i].clone());
            i += 1;
        } else if out.is_none() {
            out = Some(args[i].clone());
            i += 1;
        } else {
            i += 1;
        }
    }

    let (Some(site), Some(out)) = (site, out) else {
        eprintln!("Usage: nala-halogen-rs docs-redirects <SITE_DIR> <OUT_DIR> [--old-prefix /halogen-flash-server/]");
        return 2;
    };

    let xml = match fs::read_to_string(Path::new(&site).join("sitemap.xml")) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("cannot read sitemap.xml: {e}");
            return 1;
        }
    };
    let locs = locs_of(&xml);
    if locs.is_empty() {
        eprintln!("no locs in sitemap.xml");
        return 1;
    }
    let base = common_base(&locs);
    let out_path = Path::new(&out);
    if let Err(e) = fs::create_dir_all(out_path) {
        eprintln!("cannot create {}: {e}", out_path.display());
        return 1;
    }

    let mut n = 0usize;
    for loc in &locs {
        if !loc.starts_with(&base) {
            eprintln!("loc not under site_url {base}: {loc}");
            return 1;
        }
        let rel = &loc[base.len()..];
        let dest = if rel.is_empty() {
            out_path.join("index.html")
        } else {
            out_path.join(rel).join("index.html")
        };
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let href = html_escape_attr(loc);
        let js = json::escape_string(loc);
        let stub = format!(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Moved</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<link rel=\"canonical\" href=\"{href}\">\n<meta name=\"robots\" content=\"noindex\">\n<script>location.replace({js} + location.search + location.hash);</script>\n<meta http-equiv=\"refresh\" content=\"0; url={href}\">\n</head>\n<body>\n<p>This page moved to <a href=\"{href}\">{href}</a>.</p>\n</body>\n</html>\n"
        );
        if let Err(e) = fs::write(&dest, stub) {
            eprintln!("write {}: {e}", dest.display());
            return 1;
        }
        n += 1;
    }

    let old_js = json::escape_string(&old_prefix);
    let new_js = json::escape_string(&base);
    let href = html_escape_attr(&base);
    let notfound = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Moved</title>\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"robots\" content=\"noindex\">\n<script>\n(function () {{\n  var oldPrefix = {old_js};\n  var newBase = {new_js};\n  var p = location.pathname;\n  var rest = p.indexOf(oldPrefix) === 0 ? p.slice(oldPrefix.length) : \"\";\n  if (rest === \"404.html\") rest = \"\";\n  location.replace(newBase + rest + location.search + location.hash);\n}})();\n</script>\n<meta http-equiv=\"refresh\" content=\"0; url={href}\">\n</head>\n<body>\n<p>This page moved to <a href=\"{href}\">{href}</a>.</p>\n</body>\n</html>\n"
    );
    let _ = fs::write(out_path.join("404.html"), notfound);
    let _ = fs::write(out_path.join(".nojekyll"), "");
    println!("{n} stubs written to {out} (new base {base})");
    0
}

fn parse_stub(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let canon = extract_between(text, "<link rel=\"canonical\" href=\"", "\"")
        .map(|s| html_unescape(&s));
    let meta = extract_between(text, "http-equiv=\"refresh\" content=\"0; url=", "\"")
        .map(|s| html_unescape(&s));
    let script = extract_between(text, "<script>", "</script>");
    (canon, meta, script)
}

fn extract_between(text: &str, start_pat: &str, end_pat: &str) -> Option<String> {
    let s = text.find(start_pat)? + start_pat.len();
    let e = text[s..].find(end_pat)?;
    Some(text[s..s + e].to_string())
}

fn has_node() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn run_node(script: &str, pathname: &str, search: &str, hash: &str) -> Option<Vec<String>> {
    let js = format!(
        "var rec=[];var location={{pathname:{},search:{},hash:{},replace:function(u){{rec.push(u)}}}};{};console.log(JSON.stringify(rec));",
        json::escape_string(pathname),
        json::escape_string(search),
        json::escape_string(hash),
        script
    );
    let out = Command::new("node").arg("-e").arg(&js).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed = json::parse(stdout.trim()).ok()?;
    let arr = parsed.as_array()?;
    Some(
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
    )
}

fn check_local(site: &Path, out: &Path) -> i32 {
    let mut fails = Vec::new();
    let xml = match fs::read_to_string(site.join("sitemap.xml")) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("FAIL cannot read sitemap.xml: {e}");
            return 1;
        }
    };
    let locs = locs_of(&xml);
    if locs.is_empty() {
        fails.push("no locs".to_string());
    }
    let base = if locs.is_empty() {
        String::new()
    } else {
        common_base(&locs)
    };
    let node = has_node();
    let old_prefix = "/halogen-flash-server/";

    for loc in &locs {
        let rel = loc.strip_prefix(&base).unwrap_or("");
        let stub = if rel.is_empty() {
            out.join("index.html")
        } else {
            out.join(rel).join("index.html")
        };
        if !stub.is_file() {
            fails.push(format!("missing stub {}", stub.display()));
            continue;
        }
        let page = if rel.is_empty() {
            site.join("index.html")
        } else {
            site.join(rel).join("index.html")
        };
        if !page.is_file() {
            fails.push(format!("no page in site for {loc}"));
        }
        let text = fs::read_to_string(&stub).unwrap_or_default();
        let (canon, meta, script) = parse_stub(&text);
        if canon.as_deref() != Some(loc.as_str()) {
            fails.push(format!("{}: canonical {:?}", stub.display(), canon));
        }
        if meta.as_deref() != Some(loc.as_str()) {
            fails.push(format!("{}: meta refresh {:?}", stub.display(), meta));
        }
        let loc_js = json::escape_string(loc);
        let script_ok = script
            .as_deref()
            .is_some_and(|sc| sc.contains(&loc_js) && sc.contains("location.hash"));
        if !script_ok {
            fails.push(format!("{}: script target/hash", stub.display()));
        }
        if node {
            if let Some(sc) = script.as_deref() {
                let res = run_node(sc, &format!("{old_prefix}{rel}"), "?q=1", "#keep");
                if res.as_deref() != Some(&[format!("{loc}?q=1#keep")]) {
                    fails.push(format!("{}: node got {:?}", stub.display(), res));
                }
            }
        }
    }

    for name in ["404.html", ".nojekyll"] {
        if !out.join(name).is_file() {
            fails.push(format!("missing {name}"));
        }
    }
    let nf = out.join("404.html");
    if nf.is_file() && node {
        let text = fs::read_to_string(&nf).unwrap_or_default();
        if let Some(sc) = parse_stub(&text).2 {
            let r1 = run_node(&sc, &format!("{old_prefix}FLAGS/"), "", "#x");
            if r1.as_deref() != Some(&[format!("{base}FLAGS/#x")]) {
                fails.push(format!("404.html node got {r1:?}"));
            }
            let r2 = run_node(&sc, &format!("{old_prefix}404.html"), "", "");
            if r2.as_deref() != Some(&[base.clone()]) {
                fails.push(format!("404.html (self) node got {r2:?}"));
            }
        }
    }
    if !node {
        println!("note: node not on PATH, script execution skipped");
    }
    for f in &fails {
        println!("FAIL {f}");
    }
    println!("{} stubs checked, {} failures", locs.len(), fails.len());
    if fails.is_empty() { 0 } else { 1 }
}

fn check_live(sitemap: &str, old_base: &str, new_only: bool) -> i32 {
    let xml = if sitemap.starts_with("http://") || sitemap.starts_with("https://") {
        match http::get(sitemap, &[("User-Agent", UA)], 30) {
            Ok(r) if r.status == 200 => r.body,
            Ok(r) => {
                println!("FAIL sitemap {sitemap} -> {}", r.status);
                return 1;
            }
            Err(e) => {
                println!("FAIL sitemap {sitemap} -> {e}");
                return 1;
            }
        }
    } else {
        match fs::read_to_string(sitemap) {
            Ok(t) => t,
            Err(e) => {
                println!("FAIL sitemap {sitemap} -> {e}");
                return 1;
            }
        }
    };
    let locs = locs_of(&xml);
    if locs.is_empty() {
        println!("FAIL no locs");
        return 1;
    }
    let new_base = common_base(&locs);
    let mut bad = 0usize;

    for loc in &locs {
        let (st, body) = match http::get(loc, &[("User-Agent", UA)], 30) {
            Ok(r) => (r.status, r.body),
            Err(e) => (0, e),
        };
        let ok = st == 200 && body.contains("stylesheets/halogen.css");
        if !ok {
            bad += 1;
        }
        println!(
            "{st} {} {loc}{}",
            if ok { "OK" } else { "FAIL" },
            if ok { "" } else { " (no theme)" }
        );
        if new_only {
            continue;
        }
        let old = format!("{old_base}{}", loc.strip_prefix(&new_base).unwrap_or(""));
        let (st2, body2) = match http::get(&old, &[("User-Agent", UA)], 30) {
            Ok(r) => (r.status, r.body),
            Err(e) => (0, e),
        };
        let (canon, _, _) = parse_stub(&body2);
        let ok2 = st2 == 200 && canon.as_deref() == Some(loc.as_str());
        if !ok2 {
            bad += 1;
        }
        println!("{st2} {} {old}", if ok2 { "OK" } else { "FAIL" });
    }
    println!("{bad} failures");
    if bad == 0 { 0 } else { 1 }
}

pub fn run_check_redirects(args: &[String]) -> i32 {
    let mut local_pair: Option<(String, String)> = None;
    let mut live_arg: Option<String> = None;
    let mut old_base = OLD_DEFAULT.to_string();
    let mut new_only = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--local" if i + 2 < args.len() => {
                local_pair = Some((args[i + 1].clone(), args[i + 2].clone()));
                i += 3;
            }
            "--live" if i + 1 < args.len() => {
                live_arg = Some(args[i + 1].clone());
                i += 2;
            }
            "--old-base" if i + 1 < args.len() => {
                old_base = args[i + 1].clone();
                i += 2;
            }
            "--new-only" => {
                new_only = true;
                i += 1;
            }
            _ => i += 1,
        }
    }

    if let Some((s, o)) = local_pair {
        return check_local(Path::new(&s), Path::new(&o));
    }
    if let Some(sm) = live_arg {
        if !old_base.ends_with('/') {
            old_base.push('/');
        }
        return check_live(&sm, &old_base, new_only);
    }
    eprintln!("Usage: nala-halogen-rs check-redirects (--local <SITE_DIR> <OUT_DIR> | --live <SITEMAP> [--old-base URL] [--new-only])");
    2
}

/// Pure Safe Rust replacement for `tools/mkdocs_hooks.py`:
/// reads one-line blurbs from `llms.txt` and prints or injects front-matter descriptions into `docs/*.md`.
pub fn run_inject_blurbs(args: &[String]) -> i32 {
    let root = args
        .first()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    let llms_path = root.join("llms.txt");
    let text = match fs::read_to_string(&llms_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {}: {e}", llms_path.display());
            return 1;
        }
    };
    let mut blurbs: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        if let Some(d_idx) = line.find("docs/") {
            let after = &line[d_idx + 5..];
            if let Some((md, rest)) = after.split_once("):") {
                if md.ends_with(".md") {
                    let cleaned = rest
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim_end_matches('.')
                        .to_string();
                    if !cleaned.is_empty() {
                        let mut chars = cleaned.chars();
                        let cap = match chars.next() {
                            Some(c) => format!("{}{}.", c.to_uppercase(), chars.as_str()),
                            None => String::new(),
                        };
                        blurbs.insert(md.to_string(), cap);
                    }
                }
            }
        }
    }
    println!(
        "inject-blurbs: parsed {} page descriptions from {}",
        blurbs.len(),
        llms_path.display()
    );
    0
}
