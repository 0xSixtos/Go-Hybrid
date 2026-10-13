//! hyb — the Hybrid CLI (MVP-0 shape).
//!
//! Subcommands:
//!   check   — parse + Tier-0 check a .hy file (human or --json output)
//!   explain — offline reference page for a HYB-Exxxx code
//!   build   — the walking skeleton: parse → check → emit Go → go build

use std::process::{Command, ExitCode};

use hy_diag::{Diagnostic, Pos, Span};

/// Version pins enforced by `hyb build` (HYB-E9001). Overridable at COMPILE
/// time via env (tools/env.sh exports them); the defaults match PLAN.md's
/// environment row so a plain `cargo build` still enforces the right pins.
const PINNED_GO: &str = match option_env!("HYB_PINNED_GO") {
    Some(v) => v,
    None => "go1.26.9",
};
const PINNED_RUST: &str = match option_env!("HYB_PINNED_RUST") {
    Some(v) => v,
    None => "1.99.0",
};

/// The Go binary: HYB_GO_BIN override, else PATH (tools/env.sh puts the
/// pinned toolchain first; CI prepends /usr/local/go/bin). The version-pin
/// check below still enforces go1.26.9, so PATH lookup cannot drift silently.
fn go_bin() -> &'static str {
    option_env!("HYB_GO_BIN").unwrap_or("go")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("explain") => cmd_explain(&args[1..]),
        Some("check") => cmd_check(&args[1..]),
        Some("build") => cmd_build(&args[1..]),
        _ => {
            eprintln!("hyb — the Hybrid compiler CLI (MVP-0)");
            eprintln!();
            eprintln!("usage:");
            eprintln!("  hyb check <file.hy> [--json] [--report-managed]");
            eprintln!("  hyb explain <HYB-Exxxx | Exxxx | xxxx | machine-name>");
            eprintln!("  hyb build <file.hy> -o <bin-path> [--emit-go]");
            eprintln!();
            eprintln!("build pins: {} / rust {} (HYB-E9001 on drift)", PINNED_GO, PINNED_RUST);
            ExitCode::from(2)
        }
    }
}

fn cmd_explain(args: &[String]) -> ExitCode {
    let Some(code) = args.first() else {
        eprintln!("usage: hyb explain <code>");
        return ExitCode::from(2);
    };
    match hy_diag::Registry::global().explain(code) {
        Some(page) => {
            print!("{}", page);
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("unknown diagnostic code: {} (registry has {} codes)", code, hy_diag::Registry::global().entries().len());
            ExitCode::from(2)
        }
    }
}

fn cmd_check(args: &[String]) -> ExitCode {
    let file = args.iter().find(|a| !a.starts_with('-')).cloned();
    let json = args.iter().any(|a| a == "--json");
    let report_managed = args.iter().any(|a| a == "--report-managed");
    let Some(file) = file else {
        eprintln!("usage: hyb check <file.hy> [--json] [--report-managed]");
        return ExitCode::from(2);
    };

    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {}", file, e);
            return ExitCode::from(2);
        }
    };

    // parse
    let ast = match hy_syntax::parse_file(&file, &src) {
        Ok(ast) => ast,
        Err(diags) => {
            if json {
                let summary = hy_diag::CheckSummary {
                    file: file.clone(),
                    verdict: "reject".into(),
                    errors: diags.len(),
                    warnings: 0,
                    managed: None,
                };
                println!("{}", hy_diag::check_output_json(&diags, &summary));
            } else {
                for d in &diags {
                    println!("{}", d.render_head());
                }
            }
            return ExitCode::FAILURE;
        }
    };

    // check
    let result = hy_check::check_file(&ast);
    let errors = result.diagnostics.iter().filter(|d| d.severity == hy_diag::Severity::Error).count();
    let warnings = result.diagnostics.iter().filter(|d| d.severity == hy_diag::Severity::Warning).count();
    let summary = hy_diag::CheckSummary {
        file: file.clone(),
        verdict: result.verdict.as_str().to_string(),
        errors,
        warnings,
        managed: if report_managed { Some(result.managed.clone()) } else { None },
    };

    if json {
        println!("{}", hy_diag::check_output_json(&result.diagnostics, &summary));
    } else {
        for d in &result.diagnostics {
            println!("{}", d.render_head());
            for n in &d.notes {
                println!("  = {}", n);
            }
            if !d.fixits.is_empty() {
                println!("  = help: choose one —");
                for (i, f) in d.fixits.iter().enumerate() {
                    println!("    {}. {}", i + 1, f.description);
                }
            }
        }
        if report_managed {
            println!(
                "managed: {} block(s), {} of {} spawn/send sites resolved via managed ({:.1}%)",
                result.managed.managed_blocks,
                result.managed.sites_via_managed,
                result.managed.total_sites,
                result.managed.pct()
            );
        }
        if result.diagnostics.is_empty() {
            println!("{}: OK (0 diagnostics)", file);
        }
    }

    if errors > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn cmd_build(args: &[String]) -> ExitCode {
    let file = args.iter().find(|a| !a.starts_with('-')).cloned();
    let Some(file) = file else {
        eprintln!("usage: hyb build <file.hy> -o <bin-path> [--emit-go]");
        return ExitCode::from(2);
    };
    let out_idx = args.iter().position(|a| a == "-o");
    let out_path = out_idx
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "a.out".to_string());
    let emit_go_only = args.iter().any(|a| a == "--emit-go");

    // --- 1. Toolchain pin enforcement (HYB-E9001) ---
    // The walking skeleton's reproducibility guard (DX study: ~30 lines).
    // HYB_ALLOW_TOOLCHAIN_DRIFT=1 is the documented escape hatch for
    // experimentation; CI and the Makefile never set it.
    if std::env::var("HYB_ALLOW_TOOLCHAIN_DRIFT").ok().as_deref() != Some("1") {
        if let Some(diag) = check_toolchain_pins(&file) {
            println!("{}", diag.render_head());
            for f in &diag.fixits {
                println!("  = help: {}", f.description);
            }
            return ExitCode::from(2);
        }
    }

    // --- 2. Parse (HYB-E0002 envelope on failure) ---
    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {}", file, e);
            return ExitCode::from(2);
        }
    };
    let ast = match hy_syntax::parse_file(&file, &src) {
        Ok(ast) => ast,
        Err(diags) => {
            let summary = hy_diag::CheckSummary {
                file: file.clone(),
                verdict: "reject".into(),
                errors: diags.len(),
                warnings: 0,
                managed: None,
            };
            println!("{}", hy_diag::check_output_json(&diags, &summary));
            for d in &diags {
                println!("{}", d.render_head());
            }
            return ExitCode::FAILURE;
        }
    };

    // --- 3. Tier-0 check (verdict gates the build) ---
    let result = hy_check::check_file(&ast);
    let errors = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == hy_diag::Severity::Error)
        .count();
    if errors > 0 {
        let summary = hy_diag::CheckSummary {
            file: file.clone(),
            verdict: result.verdict.as_str().to_string(),
            errors,
            warnings: result
                .diagnostics
                .iter()
                .filter(|d| d.severity == hy_diag::Severity::Warning)
                .count(),
            managed: None,
        };
        println!("{}", hy_diag::check_output_json(&result.diagnostics, &summary));
        for d in &result.diagnostics {
            println!("{}", d.render_head());
        }
        return ExitCode::FAILURE;
    }

    // --- 4. Emit Go ---
    let go_src = match hy_emit::emit_go(&ast) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error [emit]: {}", e.message);
            return ExitCode::from(2);
        }
    };
    let go_path = format!("{}.go", out_path);
    if let Some(parent) = std::path::Path::new(&go_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&go_path, &go_src) {
        eprintln!("error writing {}: {}", go_path, e);
        return ExitCode::from(2);
    }
    println!("emitted {}", go_path);
    if emit_go_only {
        return ExitCode::SUCCESS;
    }

    // --- 5. go build ---
    let go_bin = go_bin();
    let build = Command::new(go_bin)
        .args(["build", "-o", &out_path, &go_path])
        .env("GOFLAGS", "-p=2")
        .output();
    match build {
        Ok(out) if out.status.success() => {
            println!("built {} (go {})", out_path, PINNED_GO);
            ExitCode::SUCCESS
        }
        Ok(out) => {
            eprintln!("go build failed:");
            eprint!("{}", String::from_utf8_lossy(&out.stderr));
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("error running go build: {} (looked for {})", e, go_bin);
            ExitCode::from(2)
        }
    }
}

/// Compare installed toolchains against the pins. Returns the E9001
/// diagnostic on the FIRST mismatch (go first — it gates codegen).
fn check_toolchain_pins(file: &str) -> Option<Diagnostic> {
    let span = Span::point(Pos::new(1, 1));

    let go_out = Command::new(go_bin())
        .arg("version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    match go_out {
        None => {
            return Some(Diagnostic::error(
                "HYB-E9001",
                file,
                span,
                format!("go not found — pinned {} required", PINNED_GO),
            ));
        }
        Some(v) => {
            let found = v.split_whitespace().nth(2).unwrap_or("").to_string();
            if found != PINNED_GO {
                return Some(Diagnostic::error(
                    "HYB-E9001",
                    file,
                    span,
                    format!("toolchain mismatch: found {}, pinned {}", found, PINNED_GO),
                ));
            }
        }
    }

    let rust_out = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    if let Some(v) = rust_out {
        let found = v
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_string();
        if found != PINNED_RUST {
            return Some(Diagnostic::error(
                "HYB-E9001",
                file,
                span,
                format!("toolchain mismatch: found rustc {}, pinned {}", found, PINNED_RUST),
            ));
        }
    }
    None
}
