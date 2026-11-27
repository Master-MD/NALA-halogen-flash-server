#![forbid(unsafe_code)]

//! Pure Safe Rust replacement for `tools/check_docs.py`.
//!
//! Mechanical public documentation gate:
//! 1. Every relative markdown link in `README.md`, `docs/*.md`, `AGENTS.md`, `llms.txt` resolves
//!    (file exists, and `#anchor` matches a heading in the target).
//! 2. No `docs/*.md` page links outside `docs/` via relative path.
//! 3. Every `docs/*.md` page appears in `mkdocs.yml` nav and in `llms.txt` (excluding `docs/nala/`).
//! 4. No `§` section cites or em-dash (`—`) in upstream public docs prose.
//! 5. `README.md` stays under the 40 KiB landing-page size cap.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const README_CAP: u64 = 40 * 1024;

pub fn slug(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    for ch in text.to_lowercase().chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch.is_whitespace() {
            cleaned.push(ch);
        }
    }
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in cleaned.trim().chars() {
        if ch.is_whitespace() {
            if !prev_dash {
                out.push('-');
                prev_dash = true;
            }
        } else {
            out.push(ch);
            prev_dash = false;
        }
    }
    out
}

pub fn headings(path: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    let Ok(text) = fs::read_to_string(path) else {
        return out;
    };
    let mut fence = false;
    for ln in text.lines() {
        if ln.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let hashes = ln.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&hashes) && ln.as_bytes().get(hashes) == Some(&b' ') {
            let title = ln[hashes + 1..].trim();
            if !title.is_empty() {
                out.insert(slug(title));
            }
        }
    }
    out
}

/// Extract markdown link targets `](target)` or `](target "title")` from a single line.
pub fn extract_links(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b']' && bytes[i + 1] == b'(' {
            let start = i + 2;
            let mut j = start;
            let mut depth = 0usize;
            while j < bytes.len() {
                match bytes[j] {
                    b'(' => depth += 1,
                    b')' => {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b')' {
                let inner = line[start..j].trim();
                let target = inner.split_whitespace().next().unwrap_or("");
                if !target.is_empty() {
                    out.push(target.to_string());
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn check_file(root: &Path, path: &Path, in_docs: bool, errors: &mut Vec<String>) {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let Ok(text) = fs::read_to_string(path) else {
        errors.push(format!("{rel}: cannot read file"));
        return;
    };
    let mut fence = false;
    for (idx, ln) in text.lines().enumerate() {
        let n = idx + 1;
        if ln.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        if ln.contains('§') {
            errors.push(format!("{rel}:{n}: section cite (§) in public text"));
        }
        if ln.contains('—') {
            errors.push(format!(
                "{rel}:{n}: em-dash in prose (the rule: none in public text)"
            ));
        }
        for target in extract_links(ln) {
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
                || target.starts_with('#')
            {
                continue;
            }
            if target.starts_with('/') || target.starts_with("../") || target.starts_with("./..") {
                if in_docs {
                    errors.push(format!("{rel}:{n}: relative link leaves docs/: {target}"));
                }
                continue;
            }
            let (file_part, anchor) = match target.split_once('#') {
                Some((f, a)) => (f, Some(a)),
                None => (target.as_str(), None),
            };
            let tgt: PathBuf = if file_part.is_empty() {
                path.to_path_buf()
            } else {
                path.parent().unwrap_or(root).join(file_part)
            };
            if !tgt.exists() {
                errors.push(format!("{rel}:{n}: link to missing file: {target}"));
                continue;
            }
            if let Some(anc) = anchor {
                if tgt.is_file() && !headings(&tgt).contains(&slug(anc)) {
                    errors.push(format!(
                        "{rel}:{n}: anchor not a heading in {file_part}: {anc}"
                    ));
                }
            }
        }
    }
}

pub fn run(args: &[String]) -> i32 {
    let root = if let Some(p) = args.first() {
        PathBuf::from(p)
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let docs_dir = root.join("docs");
    let mut pages: Vec<String> = match fs::read_dir(&docs_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".md") {
                    Some(name)
                } else {
                    None
                }
            })
            .collect(),
        Err(e) => {
            eprintln!("check-docs: cannot read {}: {e}", docs_dir.display());
            return 1;
        }
    };
    pages.sort();

    let mut errors = Vec::new();
    for f in ["README.md", "AGENTS.md", "llms.txt"] {
        let p = root.join(f);
        if p.exists() {
            check_file(&root, &p, false, &mut errors);
        }
    }
    for f in &pages {
        check_file(&root, &docs_dir.join(f), true, &mut errors);
    }

    let nav = fs::read_to_string(root.join("mkdocs.yml")).unwrap_or_default();
    let llms = fs::read_to_string(root.join("llms.txt")).unwrap_or_default();
    for f in &pages {
        let in_nav = nav.lines().any(|ln| {
            if let Some((_, rhs)) = ln.rsplit_once(':') {
                rhs.trim() == f
            } else {
                false
            }
        });
        if !in_nav {
            errors.push(format!("mkdocs.yml: docs/{f} is not in nav"));
        }
        if !llms.contains(&format!("docs/{f})")) {
            errors.push(format!("llms.txt: docs/{f} is not listed"));
        }
    }

    let readme_size = fs::metadata(root.join("README.md"))
        .map(|m| m.len())
        .unwrap_or(0);
    if readme_size > README_CAP {
        errors.push(format!(
            "README.md is {readme_size} bytes; cap is {README_CAP} (move prose into docs/)"
        ));
    }

    if !errors.is_empty() {
        println!("check_docs: FAIL ({})", errors.len());
        for e in &errors {
            println!("  {e}");
        }
        return 1;
    }

    println!(
        "check_docs: OK ({} docs pages, README {} bytes)",
        pages.len(),
        readme_size
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_and_links() {
        assert_eq!(slug("Bring your own GGUF!"), "bring-your-own-gguf");
        assert_eq!(
            extract_links("See [Quickstart](docs/getting-started.md) and [Measured](docs/performance.md#measured)."),
            vec!["docs/getting-started.md", "docs/performance.md#measured"]
        );
    }
}
