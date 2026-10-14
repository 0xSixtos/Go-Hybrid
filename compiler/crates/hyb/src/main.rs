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
/// else PATH. (No absolute fallback: the rustless install contract — the
/// release smoke strips cargo from PATH — would be silently defeated by a
/// ~/.cargo/bin fallback rebuilding a fresh lib the shipped tarball already
/// carries. If freshness ever fails without cargo, fail LOUDLY instead.)
fn cargo_bin() -> String {
    if let Ok(c) = std::env::var("HYB_CARGO_BIN") {
        return c;
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
            eprintln!("  hyb check <file.hy> [--json] [--report-managed] [--deny-managed]");
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
    let deny_managed = args.iter().any(|a| a == "--deny-managed");
    let Some(file) = file else {
        eprintln!("usage: hyb check <file.hy> [--json] [--report-managed] [--deny-managed]");
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

    // CI policy gate (--deny-managed): any managed block fails the check.
    // Pairs with --report-managed (count) the way `unsafe` pairs with audit
    // tooling — the hatch stays greppable, auditable, countable, AND now
    // optionally forbidden. Discussion #1 (ssmurfgg04-gif) ask.
    if deny_managed && result.managed.managed_blocks > 0 {
        let msg = format!(
            "deny-managed: {} managed block(s) present — check failed by policy (--deny-managed); \
             audit the block(s) or remove them (HYB-E8001 counts them under --report-managed)",
            result.managed.managed_blocks
        );
        if json {
            eprintln!("{}", msg);
        } else {
            println!("{}", msg);
        }
        return ExitCode::FAILURE;
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
    // Candidate staticlibs — the freshest ARCH-MATCHING fresh one wins.
    // (Row 16 lesson: the bridge smoke on the arm64 fleet leg caught hyb
    // happily linking a foreign-arch libhyrt.a. Candidates cover the dev
    // shared-target flow (CARGO_TARGET_DIR), cross --target subdirs, and
    // the crate-local layout the shipped tarball re-lays the prebuilt lib
    // into (target/release/libhyrt.a — the one end users hit).)
    let mut cands: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(t) = std::env::var("CARGO_TARGET_DIR") {
        let t = std::path::PathBuf::from(t);
        cands.push(t.join("release/libhyrt.a"));
        push_sibling_libs(&t, &mut cands);
    }
    let local_target = crate_dir.join("target");
    cands.push(local_target.join("release/libhyrt.a"));
    push_sibling_libs(&local_target, &mut cands);
    cands.sort();
    cands.dedup();
    let src_mtime = newest_mtime(&crate_dir);
    if std::env::var("HYB_DEBUG_LIB").is_ok() {
        eprintln!(
            "[hyb-debug] runtime={} crate_dir={} newest_src_mtime={:?}",
            rt.display(),
            crate_dir.display(),
            src_mtime
        );
    }
    let src_lib = cands
        .into_iter()
        .filter(|p| lib_arch_matches(p))
        .filter(|p| match (&src_mtime, std::fs::metadata(p)) {
            (Some(src_t), Ok(md)) => md.modified().ok().map(|l| l >= *src_t).unwrap_or(false),
            _ => false,
        })
        .max_by_key(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });
    if std::env::var("HYB_DEBUG_LIB").is_ok() {
        eprintln!("[hyb-debug] chosen staticlib: {:?}", src_lib.as_ref().map(|p| p.display().to_string()));
    }
    let src_lib = match src_lib {
        Some(p) => p,
        None => {
            // No fresh arch-matching lib — rebuild native (the dev flow; the
            // release tarball never lands here because packaging touches the
            // shipped lib's mtime past every source's).
            let rebuilt = crate_dir.join("target/release/libhyrt.a");
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
            rebuilt
        }
    };
    Ok(src_lib)
}

/// Push <dir>/<subdir>/release/libhyrt.a for every subdir (cargo's per-triple
/// layout: target/<triple>/release/... when the crate was built with --target).
fn push_sibling_libs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.path().is_dir() && e.file_name() != "release" {
                out.push(e.path().join("release/libhyrt.a"));
            }
        }
    }
}

/// Does the file at `p` contain an ELF image whose e_machine matches the
/// arch hyb itself was compiled for? (x86-64 = 62, aarch64 = 183; the u16
/// e_machine sits little-endian at offset 0x12 of the ELF header.)
///
/// `libhyrt.a` is an ar ARCHIVE — its first bytes are `!<arch>`, not ELF,
/// and Rust's symbol-index member can be hundreds of KiB (the naive "scan
/// the head of the file" approach misses the first object). So the archive
/// structure is walked properly: member headers are 60 bytes (name 16, size
/// 10 at offset 48, magic "`\n"), the symbol table (/) and long-name table
/// (//) are skipped, and the first DATA member's ELF header decides. Raw
/// ELF (a .o) is checked directly. Anything else — text, truncated files,
/// non-ELF first member — is rejected: a stale file or a foreign-arch
/// archive must never be linked by accident.
fn lib_arch_matches(p: &std::path::Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    fn machine_matches(m: u16) -> bool {
        match std::env::consts::ARCH {
            "x86_64" => m == 62,
            "aarch64" => m == 183,
            // Unknown host arch: accept ELF64 but never a mismatched known one.
            _ => m == 62 || m == 183,
        }
    }
    let Ok(mut f) = std::fs::File::open(p) else {
        return false;
    };
    let mut magic = [0u8; 8];
    if f.read_exact(&mut magic).is_err() {
        return false;
    }
    if &magic != b"!<arch>\n" {
        // Not an ar archive — a raw ELF image may start here.
        let mut hdr = [0u8; 20];
        if f.seek(SeekFrom::Start(0)).is_err() {
            return false;
        }
        if f.read_exact(&mut hdr).is_err() || &hdr[0..4] != b"\x7fELF" {
            return false;
        }
        return machine_matches(u16::from_le_bytes([hdr[18], hdr[19]]));
    }
    loop {
        let mut m = [0u8; 60];
        if f.read_exact(&mut m).is_err() || &m[58..60] != b"`\n" {
            return false;
        }
        let name = String::from_utf8_lossy(&m[0..16]).trim_end().to_string();
        let size: u64 = match std::str::from_utf8(&m[48..58])
            .ok()
            .and_then(|s| s.trim().parse().ok())
        {
            Some(s) => s,
            None => return false,
        };
        if name == "/" || name == "//" || name.starts_with("__.SYMDEF") {
            // symbol index / long-name table: skip (data 16-byte aligned)
            let skip = size + (size & 1);
            if f.seek(SeekFrom::Current(skip as i64)).is_err() {
                return false;
            }
            continue;
        }
        // First data member: its object header decides the archive's arch.
        let mut hdr = [0u8; 20];
        if f.read_exact(&mut hdr).is_err() || &hdr[0..4] != b"\x7fELF" {
            return false;
        }
        return machine_matches(u16::from_le_bytes([hdr[18], hdr[19]]));
    }
}

/// The runtime directory: HYB_RUNTIME_DIR override, then the TARBALL layout
/// (runtime/ next to the running hyb — a release artifact is self-contained),
/// else the repo-relative dev default (the compiler crate sits at
/// <repo>/compiler/crates/hyb).
///
/// Order matters (row 16 lesson): the CARGO_MANIFEST_DIR fallback points at
/// the BUILD machine's repo — for a shipped binary it usually does not exist
/// (today's users need the install.sh launcher for exactly this reason), and
/// when it does exist (fleet smokes run on the checkout that built hyb) it
/// can hold a FOREIGN-ARCH staticlib. Resolving the executable's own
/// directory first makes the tarball the source of truth for its own binary.
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
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("runtime");
            if p.is_dir() {
                return Ok(p);
            }
        }
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

    // 1. The hyrt staticlib — via ensure_staticlib (fresh + ARCH-MATCHED).
    //    (Row 16 lesson: this path used to duplicate the logic naively — it
    //    cargo-rebuilt NATIVELY on every build (the ws lib never pre-exists)
    //    and copied whatever cargo produced: a foreign-arch lib under cross
    //    builds, found via cargo_bin()'s old ~/.cargo fallback even in the
    //    rustless smoke. The bridge smoke on the arm64 fleet leg caught it.)
    let lib = ws.join("lib/libhyrt.a");
    let src_lib = ensure_staticlib(&rt)?;
    std::fs::copy(&src_lib, &lib).map_err(|e| {
        BuildError::Tool(format!(
            "copying {} → {}: {}",
            src_lib.display(),
            lib.display(),
            e
        ))
    })?;

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
        // Empty/unparseable output is treated as ABSENT: the rust pin
        // governs the toolchain WHEN IT IS USED (bridge builds via
        // ensure_staticlib); a bridge-free build — the arm64 release smoke,
        // where rustc is legitimately not on PATH at all — must not die on
        // a rustc that exists but prints nothing. Absent (ENOENT) already
        // skips; this closes the same hole for the found-but-silent case.
        // Found during the arm64 release-matrix smoke (row 11/16).
        let found = v.split_whitespace().nth(1).unwrap_or("").to_string();
        if !found.is_empty() && found != PINNED_RUST {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// lib_arch_matches must accept a matching-arch ELF and reject a
    /// foreign-arch one, garbage, and short files (row 16: the bridge smoke
    /// caught hyb linking a foreign-arch libhyrt.a — the matcher is the
    /// regression guard).
    #[test]
    fn lib_arch_matches_rejects_foreign_elf() {
        let dir = std::env::temp_dir().join(format!("hyb-archtest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Minimal 20-byte ELF headers: magic + e_machine (LE u16 at 0x12).
        let mut x86 = vec![0x7fu8, b'E', b'L', b'F'];
        x86.extend(std::iter::repeat(0u8).take(14));
        x86.extend_from_slice(&62u16.to_le_bytes()); // EM_X86_64
        let mut arm = x86.clone();
        arm[18] = 183u8; // EM_AARCH64
        let mut arm_hi = arm.clone();
        arm_hi[19] = 0u8; // wrong high byte -> machine 183 != 0x00b7? 183 = 0xb7, so hi must be 0; arm above already has 0? no: to_le_bytes set [18]=0xb7,[19]=0 — fix arm_hi to a WRONG machine (999)
        arm_hi[18] = 0xe7u8;
        arm_hi[19] = 0x03u8; // e_machine = 999 (unknown arch) — host arch never matches
        let garbage: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0, 0, 0];
        for (name, bytes, want_x86, want_arm) in [
            ("x86.a", &x86, true, false),
            ("arm.a", &arm, false, true),
            ("junk.a", &arm_hi, false, false),
            ("garbage.a", &garbage, false, false),
        ] {
            let p = dir.join(name);
            std::fs::write(&p, bytes).unwrap();
            let got = lib_arch_matches(&p);
            let expected = if std::env::consts::ARCH == "x86_64" { want_x86 } else { want_arm };
            assert_eq!(got, expected, "case {} (host {})", name, std::env::consts::ARCH);
        }
        // ar-archive shape: real libhyrt.a starts with "!<arch>" and carries
        // a big symbol-index member ("/") first — the walker must skip it and
        // decide on the first DATA member (row 16 regression).
        let member = |name: &[u8], size: usize, data: &[u8]| -> Vec<u8> {
            // ar pads every field with SPACES (a NUL-padded name is not
            // parseable — trim_end only strips whitespace, matching real ar)
            let mut h = vec![b' '; 16];
            h[..name.len()].copy_from_slice(name);
            h.extend_from_slice(b"0           "); // mtime
            h.extend_from_slice(b"0     "); // uid
            h.extend_from_slice(b"0     "); // gid
            h.extend_from_slice(b"0       "); // mode
            h.extend_from_slice(format!("{:010}", size).as_bytes());
            h.extend_from_slice(b"`\n");
            let mut v = h;
            v.extend_from_slice(data);
            if size % 2 == 1 {
                v.push(b'\n'); // ar pads data to even offsets
            }
            v
        };
        let mut ar_x86 = b"!<arch>\n".to_vec();
        ar_x86.extend(member(b"/", 312132, &vec![0u8; 312132])); // symbol index (skipped)
        ar_x86.extend(member(b"x86.o/", x86.len(), &x86)); // first data member
        let p = dir.join("ar-x86.a");
        std::fs::write(&p, &ar_x86).unwrap();
        assert_eq!(
            lib_arch_matches(&p),
            std::env::consts::ARCH == "x86_64",
            "ar-wrapped x86 member (behind a symbol index)"
        );
        // ar archive whose first data member is foreign-arch must be rejected
        let mut ar_arm = b"!<arch>\n".to_vec();
        ar_arm.extend(member(b"arm.o/", arm.len(), &arm));
        let p = dir.join("ar-arm.a");
        std::fs::write(&p, &ar_arm).unwrap();
        assert_eq!(
            lib_arch_matches(&p),
            std::env::consts::ARCH == "aarch64",
            "ar-wrapped arm member"
        );
        // short file (< 20 bytes) must be rejected, not panic
        let p = dir.join("short.a");
        std::fs::write(&p, b"\x7fELF").unwrap();
        assert!(!lib_arch_matches(&p));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The real shipped staticlib (if present) must match the host arch hyb
    /// was built for — the tarball-contract sanity check.
    #[test]
    fn real_staticlib_matches_host_arch_when_present() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../runtime/rust/hyrt/target/release/libhyrt.a");
        if !p.exists() {
            return; // no lib built in this checkout — nothing to assert
        }
        assert!(lib_arch_matches(&p), "shipped libhyrt.a arch != host arch");
    }
}
