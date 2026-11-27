#![forbid(unsafe_code)]

use nala_halogen_rs::{
    bench_serving, check_docs, cluster_probe, halogen_bench, redirects, site_release, version,
};
use std::env;
use std::process;

fn print_help() {
    println!(
        "nala-halogen-rs v{} — 100% Native Pure Safe Rust (Zero-Python, 0 Crates) Tooling for halogen-flash-server\n\
         \n\
         USAGE:\n\
           nala-halogen-rs <COMMAND> [ARGS...]\n\
         \n\
         COMMANDS:\n\
           bench-serving [DRAFTERS] [MAXTOK] [EFFORT] [REPS] [TEMP] [TOPP]\n\
                                    Benchmark drafters & verify greedy SHA-256 byte-identity\n\
           halogen-bench [FLAGS]    Run llama-bench-shaped pp/tg HTTP sweep (--api, -p, -n, -d, -r, --json)\n\
           check-docs [ROOT]        Mechanical docs gate (links, anchors, mkdocs nav, llms.txt, README cap)\n\
           site-release <MODE> ...  Render website/*.html from release.json or verify figures against docs\n\
           docs-redirects <S> <O>   Generate GitHub Pages redirect stubs & 404.html from sitemap.xml\n\
           check-redirects [FLAGS]  Verify redirect stubs (--local SITE OUT | --live SITEMAP)\n\
           inject-blurbs [ROOT]     Parse one-line page blurbs from llms.txt (Zero-Python mkdocs hook)\n\
           probe [--api URL]        Probe a live halogen-flash-server (/health, /v1/models) for NALA/Lemonade\n\
           sheriff-audit [ROOT]     Enforce 0.0% Python, Zero-PII, and intact LICENSE/TRADEMARKS\n\
           --version | -V           Print version (SSOT from VERSION)\n\
           --help    | -h           Print this help message",
        version()
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_help();
        process::exit(0);
    }

    let code = match args[0].as_str() {
        "-h" | "--help" | "help" => {
            print_help();
            0
        }
        "-V" | "--version" | "version" => {
            println!("nala-halogen-rs {}", version());
            0
        }
        "bench-serving" => bench_serving::run(&args[1..]),
        "halogen-bench" => halogen_bench::run(&args[1..]),
        "check-docs" => check_docs::run(&args[1..]),
        "site-release" => site_release::run(&args[1..]),
        "docs-redirects" => redirects::run_docs_redirects(&args[1..]),
        "check-redirects" => redirects::run_check_redirects(&args[1..]),
        "inject-blurbs" => redirects::run_inject_blurbs(&args[1..]),
        "probe" => cluster_probe::run_probe(&args[1..]),
        "sheriff-audit" => cluster_probe::run_sheriff_audit(&args[1..]),
        other => {
            eprintln!("Unknown command: '{other}'\nRun `nala-halogen-rs --help` for usage.");
            2
        }
    };
    process::exit(code);
}
