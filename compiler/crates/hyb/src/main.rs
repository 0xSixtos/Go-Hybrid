//! hyb — the Hybrid CLI (MVP-0 shape).
//!
//! Subcommands:
//!   check   — parse + Tier-0 check a .hy file (human or --json output)
//!   explain — offline reference page for a HYB-Exxxx code
//!   build   — MVP-0 walking skeleton (lands with the emitter)

use std::process::ExitCode;

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
            eprintln!("  hyb build <file.hy> -o <dir>");
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
    let Some(_file) = file else {
        eprintln!("usage: hyb build <file.hy> -o <dir>");
        return ExitCode::from(2);
    };
    // MVP-0 walking skeleton: parse → check → emit Go → go build.
    // Lands with the emitter (PLAN.md Phase-1 item).
    eprintln!("hyb build: not yet implemented (MVP-0 emitter pending)");
    ExitCode::from(2)
}
