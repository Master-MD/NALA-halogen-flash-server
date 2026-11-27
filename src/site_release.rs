#![forbid(unsafe_code)]

//! Pure Safe Rust replacement for `tools/site_release.py`.
//!
//! Renders `website/*.html` templates from `website/release.json` with SHA-256 CSS cache-busting,
//! and verifies every figure in `release.json` against `CHANGELOG.md` and the built docs HTML.

use crate::json::{self, JsonValue};
use crate::sha256;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

fn is_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

fn vkey(s: &str) -> (u64, u64, u64) {
    let mut it = s.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9].iter().all(|&i| b[i].is_ascii_digit())
}

fn is_fig_id(s: &str) -> bool {
    !s.is_empty()
        && s.split('.').all(|seg| {
            !seg.is_empty()
                && seg
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        })
}

fn parse_num(s: &str) -> Option<f64> {
    s.replace(',', "").parse::<f64>().ok()
}

fn decimals_of(s: &str) -> usize {
    let clean = s.replace(',', "");
    match clean.split_once('.') {
        Some((_, frac)) => frac.len(),
        None => 0,
    }
}

fn round_half_up(val: f64, dec: usize) -> f64 {
    let factor = 10f64.powi(dec as i32);
    let scaled = val * factor;
    if scaled >= 0.0 {
        (scaled + 0.5).floor() / factor
    } else {
        (scaled - 0.5).ceil() / factor
    }
}

fn html_escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn validate(data: &JsonValue) -> Vec<String> {
    let mut errs = Vec::new();
    if data.as_object().is_none() {
        return vec!["release.json: not an object".to_string()];
    }
    let ver = match data.get("version").and_then(|v| v.as_str()) {
        Some(v) if is_semver(v) => Some(v),
        _ => {
            errs.push("release.json: version must be N.N.N".to_string());
            None
        }
    };
    match data.get("release_date").and_then(|v| v.as_str()) {
        Some(d) if is_date(d) => {}
        _ => errs.push("release.json: release_date must be YYYY-MM-DD".to_string()),
    }
    let notes = match data.get("patch_notes").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => {
            errs.push("release.json: patch_notes must be a non-empty list".to_string());
            &[]
        }
    };
    let mut prev: Option<&str> = None;
    for (i, n) in notes.iter().enumerate() {
        let nv = n.get("version").and_then(|v| v.as_str());
        let Some(nv) = nv.filter(|s| is_semver(s)) else {
            errs.push(format!("release.json: patch_notes[{i}].version must be N.N.N"));
            continue;
        };
        let text_ok = n
            .get("text")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        if !text_ok {
            errs.push(format!("release.json: patch_notes[{i}].text must be non-empty"));
        }
        if i == 0 {
            if let Some(v) = ver {
                if nv != v {
                    errs.push(format!(
                        "release.json: patch_notes[0].version {nv} is not version {v}"
                    ));
                }
            }
        }
        if let Some(pr) = prev {
            if vkey(nv) >= vkey(pr) {
                errs.push(format!(
                    "release.json: patch_notes[{i}].version {nv} does not descend from {pr}"
                ));
            }
        }
        prev = Some(nv);
    }

    let figs = match data.get("figures").and_then(|v| v.as_object()) {
        Some(obj) if !obj.is_empty() => obj,
        _ => {
            errs.push("release.json: figures must be a non-empty object".to_string());
            &[]
        }
    };
    for (fid, f) in figs {
        let p = format!("release.json: figure {fid}");
        if !is_fig_id(fid) {
            errs.push(format!("{p}: bad id"));
        }
        if f.as_object().is_none() {
            errs.push(format!("{p}: not an object"));
            continue;
        }
        let val = f.get("value").and_then(|v| v.as_str());
        if val.is_none_or(|s| s.is_empty()) {
            errs.push(format!("{p}: value must be a non-empty string"));
        }
        if f.get("unit").and_then(|v| v.as_str()).is_none() {
            errs.push(format!("{p}: unit must be a string"));
        }
        for k in ["label", "conditions"] {
            if f.get(k)
                .and_then(|v| v.as_str())
                .is_none_or(|s| s.trim().is_empty())
            {
                errs.push(format!("{p}: {k} must be non-empty text"));
            }
        }
        match f.get("measured_at").and_then(|v| v.as_str()) {
            Some(m) if is_semver(m) => {
                if let Some(v) = ver {
                    if vkey(m) > vkey(v) {
                        errs.push(format!("{p}: measured_at {m} is newer than version {v}"));
                    }
                }
            }
            _ => errs.push(format!("{p}: measured_at must be N.N.N")),
        }
        let s = f.get("source");
        let src_ok = s.is_some_and(|so| {
            so.get("page").and_then(|x| x.as_str()).is_some()
                && so
                    .get("anchor")
                    .and_then(|x| x.as_str())
                    .is_some_and(|a| !a.is_empty())
        });
        if !src_ok {
            errs.push(format!("{p}: source needs a page and a non-empty anchor"));
        }
        if let Some(dv_val) = f.get("docs_value") {
            if !matches!(dv_val, JsonValue::Null) {
                match dv_val.as_str() {
                    Some(dv) if !dv.is_empty() => {
                        if let Some(v) = val.filter(|s| !s.is_empty()) {
                            match (parse_num(dv), parse_num(v)) {
                                (Some(a), Some(b)) => {
                                    let dec = decimals_of(v);
                                    if (round_half_up(a, dec) - b).abs() > 1e-9 {
                                        errs.push(format!(
                                            "{p}: docs_value {dv} does not round to value {v}"
                                        ));
                                    }
                                }
                                _ => errs.push(format!("{p}: docs_value and value must be numbers")),
                            }
                        }
                    }
                    _ => errs.push(format!("{p}: docs_value must be a non-empty string")),
                }
            }
        }
    }
    errs
}

fn load(path: &Path) -> Result<JsonValue, Vec<String>> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return Err(vec![format!("{}: {e}", path.display())]),
    };
    let mut errs = Vec::new();
    if !text.ends_with('\n') {
        errs.push(format!("{}: no trailing newline", path.display()));
    }
    let data = match json::parse(&text) {
        Ok(v) => v,
        Err(e) => return Err(vec![format!("{}: {e}", path.display())]),
    };
    errs.extend(validate(&data));
    if !errs.is_empty() {
        return Err(errs);
    }
    Ok(data)
}

fn join_versions(vs: &[String]) -> String {
    let mut uniq: Vec<String> = vs
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    uniq.sort_by_key(|v| vkey(v));
    if uniq.len() == 1 {
        uniq[0].clone()
    } else {
        format!(
            "{} and {}",
            uniq[..uniq.len() - 1].join(", "),
            uniq[uniq.len() - 1]
        )
    }
}

fn fig_text(f: &JsonValue) -> String {
    let val = f.get("value").and_then(|v| v.as_str()).unwrap_or("");
    let u = f.get("unit").and_then(|v| v.as_str()).unwrap_or("");
    if u.is_empty() {
        val.to_string()
    } else if u.starts_with('%') {
        format!("{val}{u}")
    } else {
        format!("{val} {u}")
    }
}

fn render_text(
    text: &str,
    data: &JsonValue,
    name: &str,
    used: &mut HashSet<String>,
    errs: &mut Vec<String>,
) -> String {
    let figs_slice = data
        .get("figures")
        .and_then(|v| v.as_object())
        .unwrap_or(&[]);
    let figs: HashMap<&str, &JsonValue> =
        figs_slice.iter().map(|(k, v)| (k.as_str(), v)).collect();

    let mut out = String::with_capacity(text.len());
    let mut pos = 0usize;
    let bytes = text.as_bytes();

    while pos < bytes.len() {
        if pos + 1 < bytes.len() && bytes[pos] == b'{' && bytes[pos + 1] == b'{' {
            if let Some(rel_end) = text[pos + 2..].find("}}") {
                let inner_raw = &text[pos + 2..pos + 2 + rel_end];
                let inner = inner_raw.trim();
                let line = text[..pos].bytes().filter(|&b| b == b'\n').count() + 1;
                let where_str = format!("{name}:{line}: {{{{{inner}}}}}");
                let (kind, arg) = match inner.split_once(':') {
                    Some((k, a)) => (k.trim(), a.trim()),
                    None => (inner, ""),
                };

                let replacement = if kind == "version" {
                    html_escape_text(data.get("version").and_then(|v| v.as_str()).unwrap_or(""))
                } else if kind == "patch_notes" {
                    let notes = data
                        .get("patch_notes")
                        .and_then(|v| v.as_array())
                        .unwrap_or(&[]);
                    notes
                        .iter()
                        .map(|n| {
                            let v = html_escape_text(
                                n.get("version").and_then(|x| x.as_str()).unwrap_or(""),
                            );
                            let t = html_escape_text(
                                n.get("text").and_then(|x| x.as_str()).unwrap_or(""),
                            );
                            format!("<li><h3>{v}</h3><p>{t}</p></li>")
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else if kind == "at" {
                    let prefix_dot = format!("{arg}.");
                    let vs: Vec<String> = figs_slice
                        .iter()
                        .filter(|(k, _)| k.as_str() == arg || k.starts_with(&prefix_dot))
                        .filter_map(|(_, f)| {
                            f.get("measured_at")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        })
                        .collect();
                    if vs.is_empty() {
                        errs.push(format!("{where_str}: no figure matches prefix {arg}"));
                        String::new()
                    } else {
                        html_escape_text(&join_versions(&vs))
                    }
                } else if matches!(kind, "fig" | "val" | "unit" | "bar") {
                    let (fig_key, scale) = if kind == "bar" {
                        match arg.rsplit_once(':') {
                            Some((k, s)) => (k, Some(s)),
                            None => (arg, None),
                        }
                    } else {
                        (arg, None)
                    };
                    if let Some(f) = figs.get(fig_key) {
                        used.insert(fig_key.to_string());
                        let s = match kind {
                            "fig" => fig_text(f),
                            "val" => f
                                .get("value")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            "unit" => f
                                .get("unit")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            "bar" => {
                                let raw_val =
                                    f.get("value").and_then(|v| v.as_str()).unwrap_or("");
                                match (parse_num(raw_val), scale.and_then(parse_num)) {
                                    (Some(v), Some(sc)) if sc != 0.0 => {
                                        if v > sc {
                                            errs.push(format!(
                                                "{where_str}: bar {fig_key} is {raw_val}, past its scale {}: raise the scale or the bar overflows its chart",
                                                scale.unwrap_or("")
                                            ));
                                            String::new()
                                        } else {
                                            let pct = round_half_up(v / sc * 100.0, 0) as i64;
                                            format!("{pct}%")
                                        }
                                    }
                                    _ => {
                                        errs.push(format!(
                                            "{where_str}: bar needs a numeric value and a scale, got value {raw_val} scale {:?}",
                                            scale
                                        ));
                                        String::new()
                                    }
                                }
                            }
                            _ => String::new(),
                        };
                        html_escape_text(&s)
                    } else {
                        errs.push(format!("{where_str}: unknown figure {fig_key}"));
                        String::new()
                    }
                } else {
                    errs.push(format!("{where_str}: unknown placeholder kind {kind}"));
                    String::new()
                };

                out.push_str(&replacement);
                pos = pos + 2 + rel_end + 2;
                continue;
            }
        }
        let ch = text[pos..].chars().next().unwrap();
        out.push(ch);
        pos += ch.len_utf8();
    }

    for (idx, ln) in out.lines().enumerate() {
        if ln.contains("{{") {
            let trimmed: String = ln.trim().chars().take(60).collect();
            errs.push(format!(
                "{name}:{}: an unrendered {{{{ is left: {trimmed}",
                idx + 1
            ));
        }
    }
    out
}

fn cmd_render(src: &Path, out: &Path) -> i32 {
    let data = match load(&src.join("release.json")) {
        Ok(d) => d,
        Err(errs) => return fail(&errs),
    };
    let mut used = HashSet::new();
    let mut errs = Vec::new();
    let mut rendered: BTreeMap<String, String> = BTreeMap::new();

    let mut html_files: Vec<PathBuf> = match fs::read_dir(src) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("html"))
            .collect(),
        Err(e) => return fail(&[format!("cannot read {}: {e}", src.display())]),
    };
    html_files.sort();

    for p in html_files {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        match fs::read_to_string(&p) {
            Ok(txt) => {
                let r = render_text(&txt, &data, &name, &mut used, &mut errs);
                rendered.insert(name, r);
            }
            Err(e) => errs.push(format!("{}: {e}", p.display())),
        }
    }

    if let Some(figs) = data.get("figures").and_then(|v| v.as_object()) {
        for (fid, _) in figs {
            if !used.contains(fid) {
                errs.push(format!("release.json: figure {fid} is on no page"));
            }
        }
    }
    if !errs.is_empty() {
        return fail(&errs);
    }

    let css_path = src.join("style.css");
    if css_path.is_file() {
        if let Ok(bytes) = fs::read(&css_path) {
            let tag = &sha256::hex(&bytes)[..10];
            for (name, content) in rendered.iter_mut() {
                let count = content.matches("href=\"style.css\"").count();
                if count != 1 {
                    errs.push(format!(
                        "{name}: expected one href=\"style.css\" to stamp (found {count})"
                    ));
                }
                *content = content.replace("href=\"style.css\"", &format!("href=\"style.css?v={tag}\""));
            }
        }
    }
    if !errs.is_empty() {
        return fail(&errs);
    }

    if let Err(e) = fs::create_dir_all(out) {
        return fail(&[format!("cannot create {}: {e}", out.display())]);
    }
    for (name, text) in &rendered {
        if let Err(e) = fs::write(out.join(name), text) {
            return fail(&[format!("cannot write {}: {e}", out.join(name).display())]);
        }
    }
    let ver = data.get("version").and_then(|v| v.as_str()).unwrap_or("?");
    let fig_count = data
        .get("figures")
        .and_then(|v| v.as_object())
        .map(|m| m.len())
        .unwrap_or(0);
    println!(
        "rendered {} pages from release.json {}: {} figures",
        rendered.len(),
        ver,
        fig_count
    );
    0
}

struct HtmlSections {
    chunks: Vec<String>,
    heads: Vec<(u8, String, usize)>,
    article_end: Option<usize>,
}

impl HtmlSections {
    fn parse(html: &str) -> Self {
        let mut chunks = Vec::new();
        let mut heads = Vec::new();
        let mut article_end = None;
        let mut skip = 0usize;

        let bytes = html.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'<' {
                if let Some(rel) = html[i + 1..].find('>') {
                    let tag_raw = &html[i + 1..i + 1 + rel];
                    let trimmed = tag_raw.trim();
                    if let Some(closing) = trimmed.strip_prefix('/') {
                        let name = closing
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        if name == "script" || name == "style" {
                            skip = skip.saturating_sub(1);
                        }
                        if is_block_tag(&name) {
                            chunks.push(" ".to_string());
                        }
                        if name == "article" && article_end.is_none() {
                            article_end = Some(chunks.len());
                        }
                    } else if !trimmed.starts_with('!') && !trimmed.starts_with('?') {
                        let name = trimmed
                            .split(|c: char| c.is_whitespace() || c == '/')
                            .next()
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        if name == "script" || name == "style" {
                            skip += 1;
                        }
                        if is_block_tag(&name) {
                            chunks.push(" ".to_string());
                        }
                        if name.len() == 2 && name.starts_with('h') {
                            if let Some(d) = name.as_bytes().get(1).copied() {
                                if (b'1'..=b'6').contains(&d) {
                                    if let Some(id) = extract_attr(trimmed, "id") {
                                        heads.push((d - b'0', id, chunks.len()));
                                    }
                                }
                            }
                        }
                    }
                    i = i + 1 + rel + 1;
                    continue;
                }
            }
            let next_lt = html[i..].find('<').map(|o| i + o).unwrap_or(bytes.len());
            if skip == 0 {
                let raw = &html[i..next_lt];
                chunks.push(unescape_basic_html(raw));
            }
            i = next_lt;
        }
        Self {
            chunks,
            heads,
            article_end,
        }
    }

    fn section(&self, anchor: &str) -> Option<String> {
        for (idx, (lvl, hid, pos)) in self.heads.iter().enumerate() {
            if hid != anchor {
                continue;
            }
            let mut end = self.chunks.len();
            if let Some(ae) = self.article_end {
                if ae >= *pos {
                    end = ae;
                }
            }
            for (lv2, _, p2) in &self.heads[idx + 1..] {
                if lv2 <= lvl {
                    end = end.min(*p2);
                    break;
                }
            }
            let joined = self.chunks[*pos..end].concat();
            let normalized = joined
                .replace('\u{00a0}', " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            return Some(normalized);
        }
        None
    }
}

fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "p" | "li"
            | "ul"
            | "ol"
            | "tr"
            | "td"
            | "th"
            | "table"
            | "div"
            | "br"
            | "pre"
            | "dt"
            | "dd"
            | "section"
            | "blockquote"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    )
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].to_string())
}

fn unescape_basic_html(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn contains_isolated_value(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let hb = haystack.as_bytes();
    let nb = needle.as_bytes();
    let mut start = 0usize;
    while let Some(rel) = haystack[start..].find(needle) {
        let pos = start + rel;
        let end = pos + nb.len();
        let left_bad = (pos > 0 && hb[pos - 1].is_ascii_digit())
            || (pos >= 2 && hb[pos - 2].is_ascii_digit() && matches!(hb[pos - 1], b'.' | b','));
        let right_bad = (end < hb.len() && hb[end].is_ascii_digit())
            || (end + 1 < hb.len()
                && matches!(hb[end], b'.' | b',')
                && hb[end + 1].is_ascii_digit());
        if !left_bad && !right_bad {
            return true;
        }
        start = pos + 1;
    }
    false
}

fn cmd_check(rel: &Path, docs: &Path, changelog: &Path) -> i32 {
    let data = match load(rel) {
        Ok(d) => d,
        Err(errs) => return fail(&errs),
    };
    let mut errs = Vec::new();
    let cl_text = match fs::read_to_string(changelog) {
        Ok(t) => t,
        Err(e) => return fail(&[format!("cannot read {}: {e}", changelog.display())]),
    };
    let mut tops: Vec<(u64, u64, u64)> = cl_text
        .lines()
        .filter_map(|ln| ln.strip_prefix("## "))
        .map(|s| s.trim())
        .filter(|s| is_semver(s))
        .map(vkey)
        .collect();
    let ver = data.get("version").and_then(|v| v.as_str()).unwrap_or("");
    if tops.is_empty() {
        errs.push(format!("FAIL: no version heading in {}", changelog.display()));
    } else {
        tops.sort();
        let (a, b, c) = *tops.last().unwrap();
        let newest = format!("{a}.{b}.{c}");
        if newest != ver {
            errs.push(format!(
                "FAIL: release.json version {ver} is not CHANGELOG.md's newest version, {newest}"
            ));
        }
    }

    let mut pages: HashMap<PathBuf, HtmlSections> = HashMap::new();
    let figs = data
        .get("figures")
        .and_then(|v| v.as_object())
        .unwrap_or(&[]);
    for (fid, f) in figs {
        let page = f
            .get("source")
            .and_then(|s| s.get("page"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let anchor = f
            .get("source")
            .and_then(|s| s.get("anchor"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let path = if page.is_empty() || page.ends_with('/') {
            docs.join(format!("{page}index.html"))
        } else {
            docs.join(page)
        };
        if !path.is_file() {
            errs.push(format!(
                "FAIL: figure {fid}: no page {page} in the docs build"
            ));
            continue;
        }
        let parsed = pages.entry(path.clone()).or_insert_with(|| {
            let txt = fs::read_to_string(&path).unwrap_or_default();
            HtmlSections::parse(&txt)
        });
        let Some(text) = parsed.section(anchor) else {
            errs.push(format!("FAIL: figure {fid}: no heading #{anchor} in {page}"));
            continue;
        };
        let v = f
            .get("docs_value")
            .and_then(|x| x.as_str())
            .or_else(|| f.get("value").and_then(|x| x.as_str()))
            .unwrap_or("");
        if !contains_isolated_value(&text, v) {
            errs.push(format!(
                "FAIL: figure {fid}: value \"{v}\" is not in {page}#{anchor}"
            ));
        }
    }

    if !errs.is_empty() {
        return fail(&errs);
    }
    println!(
        "site figures: {} found in their docs sections; release.json {} is CHANGELOG.md's newest: OK",
        figs.len(),
        ver
    );
    0
}

fn fail(errs: &[String]) -> i32 {
    for e in errs {
        eprintln!("{e}");
    }
    1
}

pub fn run(args: &[String]) -> i32 {
    if args.len() == 3 && args[0] == "render" {
        return cmd_render(Path::new(&args[1]), Path::new(&args[2]));
    }
    if args.len() == 4 && args[0] == "check" {
        return cmd_check(
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
        );
    }
    eprintln!("Usage:\n  nala-halogen-rs site-release render <SRC_DIR> <OUT_DIR>\n  nala-halogen-rs site-release check <RELEASE_JSON> <DOCS_BUILD_DIR> <CHANGELOG_MD>");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolated_value() {
        assert!(contains_isolated_value("decode 100 tok/s", "100"));
        assert!(!contains_isolated_value("decode 1000 tok/s", "100"));
        assert!(!contains_isolated_value("value 1.100", "100"));
        assert!(contains_isolated_value("1,983 / 1,876 tok/s", "1,983"));
    }
}
