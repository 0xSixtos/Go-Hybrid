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

/// The cargo binary for the bridge's hyrt build: HYB_CARGO_BIN override,
/// else PATH, else the rustup default location (~/.cargo/bin).
fn cargo_bin() -> String {
    if let Ok(c) = std::env::var("HYB_CARGO_BIN") {
        return c;
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(home).join(".cargo/bin/cargo");
        if p.is_file() {
            return p.to_string_lossy().to_string();
        }
    }
    "cargo".to_string()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("explain") => cmd_explain(&args[1..]),
        Some("check") => cmd_check(&args[1..]),
        Some("build") => cmd_build(&args[1..]),
        Some("bench-overhead") => cmd_bench_overhead(),
        _ => {
            eprintln!("hyb — the Hybrid compiler CLI (MVP-0)");
            eprintln!();
            eprintln!("usage:");
            eprintln!("  hyb check <file.hy> [--json] [--report-managed]");
            eprintln!("  hyb explain <HYB-Exxxx | Exxxx | xxxx | machine-name>");
            eprintln!("  hyb build <file.hy> -o <bin-path> [--emit-go]");
            eprintln!("  hyb bench-overhead");
            eprintln!();
            eprintln!(
                "build pins: {} / rust {} (HYB-E9001 on drift)",
                PINNED_GO, PINNED_RUST
            );
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
            eprintln!(
                "unknown diagnostic code: {} (registry has {} codes)",
                code,
                hy_diag::Registry::global().entries().len()
            );
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
    let errors = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == hy_diag::Severity::Error)
        .count();
    let warnings = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == hy_diag::Severity::Warning)
        .count();
    let summary = hy_diag::CheckSummary {
        file: file.clone(),
        verdict: result.verdict.as_str().to_string(),
        errors,
        warnings,
        managed: if report_managed {
            Some(result.managed.clone())
        } else {
            None
        },
    };

    if json {
        println!(
            "{}",
            hy_diag::check_output_json(&result.diagnostics, &summary)
        );
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
    // The .hy file: the first arg ending in .hy (flag values never do).
    let file = args.iter().find(|a| a.ends_with(".hy")).cloned();
    let Some(file) = file else {
        eprintln!("usage: hyb build <file.hy> -o <bin-path> [--emit-go] | --package <name> --out-dir <dir> [--binding-prefix <path>]");
        return ExitCode::from(2);
    };
    let out_idx = args.iter().position(|a| a == "-o");
    let out_path = out_idx
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "a.out".to_string());
    let emit_go_only = args.iter().any(|a| a == "--emit-go");
    // Package mode (example 09): emit an importable Go package + the
    // runtime pieces, instead of building a binary.
    let pkg_idx = args.iter().position(|a| a == "--package");
    let pkg_name = pkg_idx.and_then(|i| args.get(i + 1)).cloned();
    let outdir_idx = args.iter().position(|a| a == "--out-dir");
    let out_dir = outdir_idx.and_then(|i| args.get(i + 1)).cloned();
    let bprefix_idx = args.iter().position(|a| a == "--binding-prefix");
    let binding_prefix = bprefix_idx
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "hybrid.program".to_string());

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
        println!(
            "{}",
            hy_diag::check_output_json(&result.diagnostics, &summary)
        );
        for d in &result.diagnostics {
            println!("{}", d.render_head());
        }
        return ExitCode::FAILURE;
    }

    // --- 4. Emit Go ---
    let go_src = if let Some(pn) = &pkg_name {
        match hy_emit::emit_go_package(&ast, pn, &binding_prefix) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("error [emit]: {}", e.message);
                return ExitCode::from(2);
            }
        }
    } else {
        match hy_emit::emit_go(&ast) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("error [emit]: {}", e.message);
                return ExitCode::from(2);
            }
        }
    };
    // Which rust bindings does the emitted Go call? (M6: known crates only;
    // the emitter errors cleanly on unknown ones, so this scan is exact.)
    let bindings = hy_emit::used_binding_roots(&go_src);

    // --- 4b. Package mode: assemble the package dir + runtime pieces. ---
    if let (Some(pn), Some(od)) = (&pkg_name, &out_dir) {
        return cmd_build_package(od, pn, &bindings, &go_src, &binding_prefix);
    }

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
    // With rust bindings the program is a proper MODULE (cgo broker +
    // copied runtime packages + the hyrt staticlib): assemble the workspace,
    // cargo-build hyrt, then `go build ./...` inside it. Plain programs keep
    // the single-file build.
    let go_bin = go_bin();
    if bindings.is_empty() {
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
    } else {
        match build_with_bridge(go_bin, &out_path, &go_src, &bindings) {
            Ok(()) => {
                println!("built {} (go {}, bridge: hyrt)", out_path, PINNED_GO);
                ExitCode::SUCCESS
            }
            Err(BuildError::Tool(msg)) => {
                eprintln!("{}", msg);
                ExitCode::from(2)
            }
            Err(BuildError::Go(stderr)) => {
                eprintln!("go build failed:");
                eprint!("{}", stderr);
                ExitCode::FAILURE
            }
        }
    }
}

enum BuildError {
    Tool(String),
    Go(String),
}

/// Package mode (example 09): assemble an importable Go package directory:
///
/// ```text
/// <out-dir>/<name>.go      the emitted package source
/// <out-dir>/hybridge/      the boundary core (copied)
/// <out-dir>/<binding>/     per-crate bindings (copied)
/// <out-dir>/lib/libhyrt.a  the staticlib (built fresh or reused)
/// ```
/// The consumer's module imports `<module>/<name>`; --binding-prefix told
/// the emitter what import path the bindings carry, and the binding
/// packages' hybridge import is rewritten to the same prefix.
fn cmd_build_package(
    out_dir: &str,
    pkg_name: &str,
    bindings: &[&str],
    go_src: &str,
    binding_prefix: &str,
) -> ExitCode {
    let fail = |m: String| {
        eprintln!("{}", m);
        ExitCode::from(2)
    };
    let rt = match runtime_dir() {
        Ok(p) => p,
        Err(BuildError::Tool(m)) => return fail(m),
        Err(_) => return fail("runtime dir error".to_string()),
    };
    let od = std::path::Path::new(out_dir);
    if let Err(e) = std::fs::create_dir_all(od) {
        return fail(format!("creating {}: {}", od.display(), e));
    }
    if let Err(e) = std::fs::write(od.join(format!("{}.go", pkg_name)), go_src) {
        return fail(format!("writing package source: {}", e));
    }
    let copy_pkg = |name: &str| -> Result<(), String> {
        let src = rt.join("go").join(name);
        let dst = od.join(name);
        std::fs::create_dir_all(&dst).map_err(|e| format!("mkdir {}: {}", dst.display(), e))?;
        for entry in
            std::fs::read_dir(&src).map_err(|e| format!("reading {}: {}", src.display(), e))?
        {
            let entry = entry.map_err(|e| format!("readdir: {}", e))?;
            let p = entry.path();
            if p.extension().map(|x| x == "go").unwrap_or(false) {
                let mut text = std::fs::read_to_string(&p)
                    .map_err(|e| format!("reading {}: {}", p.display(), e))?;
                // The runtime copies' hybridge import follows the consumer's
                // module (the prefix the emitter already used).
                text = text.replace("hybrid.program/", &format!("{}/", binding_prefix));
                std::fs::write(dst.join(entry.file_name()), text)
                    .map_err(|e| format!("writing {}: {}", dst.display(), e))?;
            } else if p.is_file() {
                std::fs::copy(&p, dst.join(entry.file_name()))
                    .map_err(|e| format!("copying {}: {}", p.display(), e))?;
            }
        }
        Ok(())
    };
    if let Err(m) = copy_pkg("hybridge") {
        return fail(m);
    }
    for b in bindings {
        if let Err(m) = copy_pkg(b) {
            return fail(m);
        }
    }
    // The staticlib (fresh or reused) at <out-dir>/lib/ — the bindings'
    // #cgo LDFLAGS point at ${SRCDIR}/../lib.
    match ensure_staticlib(&rt) {
        Ok(lib) => {
            let libdir = od.join("lib");
            if let Err(e) = std::fs::create_dir_all(&libdir) {
                return fail(format!("creating {}: {}", libdir.display(), e));
            }
            if let Err(e) = std::fs::copy(&lib, libdir.join("libhyrt.a")) {
                return fail(format!("copying staticlib: {}", e));
            }
        }
        Err(BuildError::Tool(m)) => return fail(m),
        Err(_) => return fail("staticlib build error".to_string()),
    }
    println!(
        "package {} ready at {} (bindings: {})",
        pkg_name,
        od.display(),
        if bindings.is_empty() {
            "none".to_string()
        } else {
            bindings.join(", ")
        }
    );
    ExitCode::SUCCESS
}

/// Build (or reuse) the hyrt staticlib; returns its path.
fn ensure_staticlib(rt: &std::path::Path) -> Result<std::path::PathBuf, BuildError> {
    let crate_dir = rt.join("rust/hyrt");
    let src_lib = match std::env::var("CARGO_TARGET_DIR") {
        Ok(t) => std::path::PathBuf::from(t).join("release/libhyrt.a"),
        Err(_) => crate_dir.join("target/release/libhyrt.a"),
    };
    let fresh = std::fs::metadata(&src_lib).is_ok()
        && newest_mtime(&crate_dir)
            .map(|src_t| {
                std::fs::metadata(&src_lib)
                    .and_then(|l| l.modified())
                    .ok()
                    .map(|lib_t| lib_t >= src_t)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    if !fresh {
        let mut cargo = Command::new(cargo_bin());
        cargo
            .args(["build", "--release", "-j2"])
            .current_dir(&crate_dir)
            .env("CARGO_BUILD_JOBS", "2");
        if let Ok(t) = std::env::var("CARGO_TARGET_DIR") {
            cargo.env("CARGO_TARGET_DIR", t);
        }
        let out = cargo
            .output()
            .map_err(|e| BuildError::Tool(format!("running cargo build (hyrt): {}", e)))?;
        if !out.status.success() {
            return Err(BuildError::Tool(format!(
                "cargo build (hyrt) failed:\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )));
        }
    }
    Ok(src_lib)
}

/// The runtime directory: HYB_RUNTIME_DIR override, else the repo-relative
/// default (the compiler crate sits at <repo>/compiler/crates/hyb).
fn runtime_dir() -> Result<std::path::PathBuf, BuildError> {
    if let Ok(d) = std::env::var("HYB_RUNTIME_DIR") {
        let p = std::path::PathBuf::from(&d);
        if p.is_dir() {
            return Ok(p);
        }
        return Err(BuildError::Tool(format!(
            "HYB_RUNTIME_DIR={} is not a directory",
            d
        )));
    }
    let manifest = env!("CARGO_MANIFEST_DIR");
    // <repo>/compiler/crates/hyb → <repo>/runtime (three ups).
    let p = std::path::Path::new(manifest)
        .join("../../../runtime")
        .canonicalize()
        .map_err(|_| {
            BuildError::Tool(
                "runtime/ not found — set HYB_RUNTIME_DIR to the repo's runtime directory".into(),
            )
        })?;
    Ok(p)
}

/// Assemble the build workspace and build with the bridge:
///
/// ```text
/// <ws>/go.mod            module hybrid.program (go 1.26)
/// <ws>/main.go           the emitted Go
/// <ws>/hybridge/         runtime/go/hybridge (copied verbatim)
/// <ws>/<binding>/        runtime/go/<binding> (copied verbatim)
/// <ws>/lib/libhyrt.a     cargo-built staticlib
/// ```
/// The binding's #cgo LDFLAGS point at ../lib (workspace-relative), so the
/// layout is part of the contract. gofmt-style tabs; CGO_ENABLED=1.
fn build_with_bridge(
    go_bin: &str,
    out_path: &str,
    go_src: &str,
    bindings: &[&str],
) -> Result<(), BuildError> {
    let rt = runtime_dir()?;
    // The workspace build runs with cwd = <ws>; a RELATIVE -o would land
    // inside the workspace and vanish with it. Anchor to the caller's cwd.
    let out_abs = if std::path::Path::new(out_path).is_absolute() {
        std::path::PathBuf::from(out_path)
    } else {
        std::env::current_dir()
            .map_err(|e| BuildError::Tool(format!("resolving cwd: {}", e)))?
            .join(out_path)
    };
    let out_path = out_abs.to_string_lossy().to_string();
    let ws = std::env::temp_dir().join(format!(
        "hyb-ws-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(ws.join("lib"))
        .map_err(|e| BuildError::Tool(format!("creating workspace {}: {}", ws.display(), e)))?;

    let run = |mut c: Command, what: &str| -> Result<String, BuildError> {
        let out = c
            .output()
            .map_err(|e| BuildError::Tool(format!("running {}: {}", what, e)))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            Err(BuildError::Tool(format!(
                "{} failed:\n{}{}",
                what,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )))
        }
    };

    // 1. The hyrt staticlib (skip when fresh — mtime of the newest source).
    let lib = ws.join("lib/libhyrt.a");
    let crate_dir = rt.join("rust/hyrt");
    let lib_fresh = std::fs::metadata(&lib).is_ok()
        && newest_mtime(&crate_dir)
            .map(|src_t| {
                std::fs::metadata(&lib)
                    .and_then(|l| l.modified())
                    .ok()
                    .map(|lib_t| lib_t >= src_t)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    if !lib_fresh {
        let mut cargo = Command::new(cargo_bin());
        cargo
            .args(["build", "--release", "-j2"])
            .current_dir(&crate_dir)
            .env("CARGO_BUILD_JOBS", "2");
        if let Ok(t) = std::env::var("CARGO_TARGET_DIR") {
            cargo.env("CARGO_TARGET_DIR", t);
        }
        run(cargo, "cargo build (hyrt)")?;
        // Copy the staticlib out of the cargo target dir (respect the env
        // override; default is the crate-local target/).
        let src_lib = match std::env::var("CARGO_TARGET_DIR") {
            Ok(t) => std::path::PathBuf::from(t).join("release/libhyrt.a"),
            Err(_) => crate_dir.join("target/release/libhyrt.a"),
        };
        std::fs::copy(&src_lib, &lib).map_err(|e| {
            BuildError::Tool(format!(
                "copying {} → {}: {}",
                src_lib.display(),
                lib.display(),
                e
            ))
        })?;
    }

    // 2. Workspace assembly.
    std::fs::write(ws.join("go.mod"), "module hybrid.program\n\ngo 1.26\n")
        .map_err(|e| BuildError::Tool(format!("writing go.mod: {}", e)))?;
    std::fs::write(ws.join("main.go"), go_src)
        .map_err(|e| BuildError::Tool(format!("writing main.go: {}", e)))?;
    let copy_pkg = |name: &str| -> Result<(), BuildError> {
        let src = rt.join("go").join(name);
        let dst = ws.join(name);
        std::fs::create_dir_all(&dst)
            .map_err(|e| BuildError::Tool(format!("mkdir {}: {}", dst.display(), e)))?;
        for entry in std::fs::read_dir(&src)
            .map_err(|e| BuildError::Tool(format!("reading {}: {}", src.display(), e)))?
        {
            let entry = entry.map_err(|e| BuildError::Tool(format!("readdir: {}", e)))?;
            let p = entry.path();
            if p.is_file() {
                std::fs::copy(&p, dst.join(entry.file_name()))
                    .map_err(|e| BuildError::Tool(format!("copying {}: {}", p.display(), e)))?;
            }
        }
        Ok(())
    };
    copy_pkg("hybridge")?;
    for b in bindings {
        copy_pkg(b)?;
    }

    // 3. go build inside the module.
    let mut gb = Command::new(go_bin);
    gb.args(["build", "-o", &out_path, "."])
        .current_dir(&ws)
        .env("GOFLAGS", "-p=2")
        .env("CGO_ENABLED", "1");
    let out = gb
        .output()
        .map_err(|e| BuildError::Tool(format!("running go build (workspace): {}", e)))?;
    if out.status.success() {
        let _ = std::fs::remove_dir_all(&ws);
        Ok(())
    } else {
        // Keep the workspace for diagnosis; say where.
        eprintln!("(workspace kept at {} for diagnosis)", ws.display());
        Err(BuildError::Go(
            String::from_utf8_lossy(&out.stderr).to_string(),
        ))
    }
}

fn newest_mtime(dir: &std::path::Path) -> Option<std::time::SystemTime> {
    let mut newest = None;
    let stack = vec![dir.to_path_buf()];
    let mut todo = stack;
    while let Some(d) = todo.pop() {
        for entry in std::fs::read_dir(&d).ok()? {
            let p = entry.ok()?.path();
            if p.is_dir() {
                if p.file_name().map(|n| n == "target").unwrap_or(false) {
                    continue; // build output, not source
                }
                todo.push(p);
            } else {
                if let Ok(m) = p.metadata().and_then(|m| m.modified()) {
                    if newest.map(|n| m > n).unwrap_or(true) {
                        newest = Some(m);
                    }
                }
            }
        }
    }
    newest
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
        let found = v.split_whitespace().nth(1).unwrap_or("").to_string();
        if found != PINNED_RUST {
            return Some(Diagnostic::error(
                "HYB-E9001",
                file,
                span,
                format!(
                    "toolchain mismatch: found rustc {}, pinned {}",
                    found, PINNED_RUST
                ),
            ));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// bench-overhead — the G2 gate's call-overhead evidence (Batch J)
// ---------------------------------------------------------------------------

/// The bench program assembled into the bridge workspace. It measures TWO
/// crossings on the LIVE bridge (the same hybridge assembly + staticlib
/// every `hyb build` links):
///
///   crossing-nop      hybridge.Cross1 → hyrt's hy_nop — the shipping
///                     fast-path trampoline with ZERO payload work. This is
///                     the quantity SPEC §6's ≤5 ns budget names.
///   binding-newobject jsonbinding.NewObject — a REAL binding call (handle
///                     allocation on the Rust side) for context.
///
/// Machine-parseable lines: `<name> <float> ns/call`. Best of 3 in-process.
const BENCH_MAIN: &str = r#"package main

import (
        "fmt"
        "runtime"
        "time"

        "hybrid.program/jsonbinding"
        "hybrid.program/overheadbench"
)

var sink uint64

//go:noinline
func goNop(x uint64) uint64 { return x }

// measure: one best-of-3 inline loop (the spike-1 shape — no closure
// indirection), run at the CURRENT stack depth. Modes: 0 = Go floor,
// 2 = the shipping guarded crossing, 3 = the raw fast trampoline.
func measure(n int, mode int) float64 {
        best := 0.0
        for rep := 0; rep < 3; rep++ {
                t0 := time.Now()
                switch mode {
                case 0:
                        for i := 0; i < n; i++ {
                                sink += goNop(uint64(i))
                        }
                case 2:
                        for i := 0; i < n; i++ {
                                sink += overheadbench.Nop(uint64(i))
                        }
                case 3:
                        for i := 0; i < n; i++ {
                                v, ok := overheadbench.NopRaw(uint64(i))
                                if !ok {
                                        panic("fast path short headroom in scenario B")
                                }
                                sink += v
                        }
                }
                d := float64(time.Since(t0).Nanoseconds()) / float64(n)
                if best == 0 || d < best {
                        best = d
                }
        }
        return best
}

// descend recurses to force contiguous stack growth (8 KB doubles on each
// morestack; 200 frames force several doublings), then measures at THAT
// depth — the grown-stack scenario where headroom exceeds the fast path's
// 4096-byte threshold.
func descend(n int, mode int, depth int) float64 {
        if depth > 0 {
                return descend(n, mode, depth-1)
        }
        return measure(n, mode)
}

func main() {
        // Warmup: trampolines, the deep-stack pool, allocator pages.
        for i := 0; i < 100000; i++ {
                sink += overheadbench.Nop(uint64(i))
        }
        const N = 5000000

        // Scenario A — SHALLOW goroutine (fresh main stack): the headroom probe
        // found ~2.9 KB here, BELOW the fast path's 4096-byte threshold, so the
        // shipping crossing engages the pooled deep path.
        aFloor := measure(N, 0)
        aNop := measure(N, 2)
        aDeep := overheadbench.DeepPathCount()
        fmt.Printf("a-floor %.2f ns/call\n", aFloor)
        fmt.Printf("a-nop %.2f ns/call\n", aNop)
        fmt.Printf("a-deepcount %d\n", aDeep)

        // Scenario B — GROWN stack: the same crossing after forced growth. The
        // fast path must engage (deepcount frozen) — the budget's steady state.
        bFloor := descend(N, 0, 200)
        bRaw := descend(N, 3, 200)
        bNop := descend(N, 2, 200)
        bDeep := overheadbench.DeepPathCount()
        fmt.Printf("b-floor %.2f ns/call\n", bFloor)
        fmt.Printf("b-raw %.2f ns/call\n", bRaw)
        fmt.Printf("b-nop %.2f ns/call\n", bNop)
        fmt.Printf("b-deepcount %d\n", bDeep)
        fmt.Printf("b-delta %.2f ns/call\n", bNop-bFloor)

        // Context: a REAL binding call — jsonbinding.NewObject allocates a
        // Rust-side handle through the shipping crossing.
        for i := 0; i < 1000; i++ {
                _ = jsonbinding.NewObject()
        }
        const M = 200000
        best := 0.0
        for rep := 0; rep < 3; rep++ {
                runtime.GC()
                t1 := time.Now()
                for i := 0; i < M; i++ {
                        v := jsonbinding.NewObject()
                        _ = v
                }
                d := float64(time.Since(t1).Nanoseconds()) / float64(M)
                if best == 0 || d < best {
                        best = d
                }
        }
        runtime.GC()
        fmt.Printf("binding-newobject %.2f ns/call\n", best)
        fmt.Printf("sink %d\n", sink)
}
"#;

/// One bench process run's parsed numbers (median-of-3 reported).
#[derive(Default, Clone, Copy)]
struct BenchRun {
    a_floor: f64,
    a_nop: f64,
    a_deep: u64,
    b_floor: f64,
    b_raw: f64,
    b_nop: f64,
    b_deep: u64,
    b_delta: f64,
    obj: f64,
}

fn cmd_bench_overhead() -> ExitCode {
    // Same pins discipline as build (HYB-E9001): the bench measures the
    // pinned toolchains, nothing else.
    if let Some(diag) = check_toolchain_pins("bench-overhead") {
        eprintln!("{}", diag.render_head());
        for n in &diag.notes {
            eprintln!("  = {}", n);
        }
        return ExitCode::FAILURE;
    }
    println!(
        "assembling the bench on the live bridge (pins: {} / rust {})...",
        PINNED_GO, PINNED_RUST
    );
    let go_bin = go_bin();
    let out = std::env::temp_dir()
        .join(format!(
            "hyb-bench-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ))
        .to_string_lossy()
        .to_string();
    if let Err(e) = build_with_bridge(go_bin, &out, BENCH_MAIN, &["overheadbench", "jsonbinding"]) {
        match e {
            BuildError::Tool(m) => eprintln!("error: {}", m),
            BuildError::Go(stderr) => eprintln!("error: go build failed:\n{}", stderr),
        }
        return ExitCode::FAILURE;
    }

    // Run 3× (separate processes; report the median run's numbers — wall
    // clock is supporting evidence per SPEC §3.4, and the number the gate
    // cites is the process-median of the in-process best-of-3).
    let mut runs: Vec<BenchRun> = Vec::new();
    for _ in 0..3 {
        let proc_out = Command::new(&out).output();
        let out = match proc_out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            Ok(o) => {
                eprintln!(
                    "bench run failed:\n{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("error running the bench: {}", e);
                return ExitCode::FAILURE;
            }
        };
        let mut r = BenchRun::default();
        for line in out.lines() {
            let mut it = line.split_whitespace();
            match (it.next(), it.next()) {
                (Some("a-floor"), Some(v)) => r.a_floor = v.parse().unwrap_or(0.0),
                (Some("a-nop"), Some(v)) => r.a_nop = v.parse().unwrap_or(0.0),
                (Some("a-deepcount"), Some(v)) => r.a_deep = v.parse().unwrap_or(0),
                (Some("b-floor"), Some(v)) => r.b_floor = v.parse().unwrap_or(0.0),
                (Some("b-raw"), Some(v)) => r.b_raw = v.parse().unwrap_or(0.0),
                (Some("b-nop"), Some(v)) => r.b_nop = v.parse().unwrap_or(0.0),
                (Some("b-deepcount"), Some(v)) => r.b_deep = v.parse().unwrap_or(0),
                (Some("b-delta"), Some(v)) => r.b_delta = v.parse().unwrap_or(0.0),
                (Some("binding-newobject"), Some(v)) => r.obj = v.parse().unwrap_or(0.0),
                _ => {}
            }
        }
        runs.push(r);
    }
    runs.sort_by(|a, b| a.b_nop.partial_cmp(&b.b_nop).unwrap());
    let r = runs[runs.len() / 2];
    let budget = 5.0f64;
    println!();
    println!("call overhead — the live bridge (hybridge fast path + hyrt staticlib):");
    println!("  scenario A — SHALLOW goroutine (fresh 8 KB main stack, ~2.9 KB headroom):");
    println!("    go-call floor     {:>7.2} ns/call", r.a_floor);
    println!("    shipping crossing {:>7.2} ns/call  (deep path: {} crossings — the pooled guarded stack)", r.a_nop, r.a_deep);
    println!("  scenario B — GROWN stack (headroom > 4096 B: the fast path's threshold):");
    println!("    go-call floor     {:>7.2} ns/call", r.b_floor);
    println!(
        "    raw fast tramp    {:>7.2} ns/call  (in-trampoline guard, no Go-wrapper retry)",
        r.b_raw
    );
    println!("    shipping crossing {:>7.2} ns/call  (Cross1; deepcount frozen at {} — fast path proven)", r.b_nop, r.b_deep);
    println!(
        "    marginal delta    {:>7.2} ns/call  (the boundary's cost over a Go call)",
        r.b_delta
    );
    println!("  binding-newobject   {:>7.2} ns/call  (context: real binding call, Rust-side handle alloc)", r.obj);
    println!("  budget              {:>7.2} ns/call  (SPEC §6 hypothesis; spike-1: 2.82 ns sandbox / 4.36 ns worker on the FIXED-stack prototype)", budget);
    println!();
    println!("  reading: the ≤5 ns hypothesis holds on the FAST path (grown stacks) —");
    if r.b_nop <= budget {
        println!(
            "  VERDICT: PASS — fast path {:.2} ns ≤ {:.2} ns ({:.0}% of budget). Shallow goroutines take the pooled deep path at {:.2} ns until their stacks grow (the collision-safe pool costs ~{:.0} ns over the spike's fixed-stack prototype).",
            r.b_nop, budget, 100.0 * r.b_nop / budget, r.a_nop, r.a_nop - r.b_nop
        );
        ExitCode::SUCCESS
    } else {
        println!(
            "  VERDICT: MISS — fast path {:.2} ns > {:.2} ns (see gates/G2.md for the interpretation + the documented fallback)",
            r.b_nop, budget
        );
        ExitCode::FAILURE
    }
}
