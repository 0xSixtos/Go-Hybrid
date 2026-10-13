//! hy-emit tests — golden-shape assertions over the emitted Go.
//!
//! Discipline: the two MVP-0 emitter bugs recovered from the crashed session
//! have named regression tests (bug-1: emission order; bug-2: make type).
//! Full end-to-end verification (go build + run + expected.txt diff) lives in
//! tools/run_examples.sh — these tests pin the emission SHAPES.

use hy_syntax::parse_file;

use super::emit_go;

fn emit(src: &str) -> String {
    let file = parse_file("t.hy", src).expect("parse");
    emit_go(&file).expect("emit")
}

fn emit_err(src: &str) -> String {
    let file = parse_file("t.hy", src).expect("parse");
    match emit_go(&file) {
        Ok(g) => panic!("expected unsupported error, got:\n{}", g),
        Err(e) => e.message,
    }
}

// ---------------------------------------------------------------------------
// The two recovered MVP-0 bugs — named regression tests
// ---------------------------------------------------------------------------

#[test]
fn bug1_struct_first_file_emits_imports_before_types() {
    // Struct declared FIRST (like example 03) — Go requires imports
    // immediately after `package`; the old single-pass emitter wrote `type`
    // decls before the import block.
    let go = emit("struct job {\n    input int\n}\n\nfn main() {\n    fmt.println(\"hi\")\n}\n");
    let pkg = go.find("package main").unwrap();
    let imp = go.find("import (").unwrap();
    let ty = go.find("type job struct").unwrap();
    assert!(pkg < imp && imp < ty, "order must be package < import < type:\n{}", go);
    assert!(go.contains("\"fmt\""), "fmt imported:\n{}", go);
}

#[test]
fn bug2_make_chan_renders_bare_type_not_value_literal() {
    // `make(chan job, 16)` — the parser encodes the type argument as an empty
    // StructLit; rendering it as `job{}` inside make() is invalid Go.
    let go = emit("struct job {\n    input int\n}\n\nfn main() {\n    jobs := make(chan job, 16)\n}\n");
    assert!(go.contains("make(chan job, 16)"), "bare type in make:\n{}", go);
    assert!(!go.contains("make(chan job{}, 16)"), "value literal leaked:\n{}", go);
}

#[test]
fn bug2_make_slice_types() {
    let go = emit("fn main() {\n    buf := make([]byte, 64 * 1024)\n}");
    assert!(go.contains("make([]byte, 64 * 1024)"), ":\n{}", go);
}

#[test]
fn map_literal_zero_value_is_direct_composite() {
    // The corpus spells empty maps as `map[string]int{}` (the parser surface
    // has no `make(map…)` encoding — maps are composite literals).
    let go = emit("fn main() {\n    m := map[string]int{}\n}");
    assert!(go.contains("m := map[string]int{}"), ":\n{}", go);
}

// ---------------------------------------------------------------------------
// Example 01 — hello (interpolation, printf, range with index)
// ---------------------------------------------------------------------------

#[test]
fn example01_golden_shape() {
    let src = r#"
fn main() {
    name := "Hybrid"
    count := 0
    count += 1
    fmt.println("hello, {name}!")
    fmt.printf("count=%d ok=%v\n", count, count == 1)
    let langs = ["Go", "Rust", "Hybrid"]
    for i, l in langs {
        fmt.println("{i}: {l}")
    }
}
"#;
    let go = emit(src);
    assert!(go.contains("name := \"Hybrid\""), ":\n{}", go);
    assert!(go.contains("count += 1"), ":\n{}", go);
    assert!(go.contains("fmt.Println(fmt.Sprintf(\"hello, %v!\", name))"), "interp → Sprintf:\n{}", go);
    assert!(go.contains("fmt.Printf(\"count=%d ok=%v\\n\", count, count == 1)"), "printf passthrough:\n{}", go);
    assert!(go.contains("langs := []string{\"Go\", \"Rust\", \"Hybrid\"}"), "inferred string slice:\n{}", go);
    assert!(go.contains("for i, l := range langs {"), "2-var range:\n{}", go);
    assert!(go.contains("fmt.Println(fmt.Sprintf(\"%v: %v\", i, l))"), ":\n{}", go);
}

// ---------------------------------------------------------------------------
// Example 02 — cli-cat (wrapper, ?-desugar, defer, byte hint, eprintln)
// ---------------------------------------------------------------------------

fn example02_src() -> String {
    r#"
fn main() (code int) {
    args := os.args()
    if len(args) < 2 {
        fmt.eprintln("usage: cat <file>")
        return 1
    }
    f := os.open(args[1])?
    defer f.close()
    data := io.read_all(f)?
    fmt.print(data)
    return 0
}
"#
    .to_string()
}

#[test]
fn example02_wrapper_shape_and_exit_contract() {
    let go = emit(&example02_src());
    assert!(go.contains("func main() {"), "wrapper main:\n{}", go);
    assert!(
        go.contains("func() (code int, hyErr error) {"),
        "closure carries named results + hidden error:\n{}",
        go
    );
    assert!(go.contains("code, hyErr := func()"), "receiver vars:\n{}", go);
    assert!(
        go.contains("fmt.Fprintln(os.Stderr, filepath.Base(os.Args[0])+\":\", hyErr)"),
        "E6 stderr line:\n{}",
        go
    );
    assert!(go.contains("os.Exit(1)"), ":\n{}", go);
    assert!(go.contains("os.Exit(code)"), ":\n{}", go);
}

#[test]
fn example02_try_desugar_and_defer() {
    let go = emit(&example02_src());
    assert!(
        go.contains("args := os.Args"),
        "os.args() → os.Args, parens dropped:\n{}",
        go
    );
    assert!(
        go.contains("f, hyErr0 := os.Open(args[1])"),
        "?-desugar binds temp error:\n{}",
        go
    );
    assert!(
        go.contains("hyErr = hyErr0") && go.contains("return"),
        "error path assigns + returns:\n{}",
        go
    );
    assert!(go.contains("defer f.Close()"), "foreign method normalized:\n{}", go);
    assert!(
        go.contains("data, hyErr1 := io.ReadAll(f)"),
        "second desugar, unique temp:\n{}",
        go
    );
    assert!(
        go.contains("os.Stdout.Write(data)"),
        "byte-binding hint routes print to Write:\n{}",
        go
    );
    assert!(go.contains("return 1, nil") && go.contains("return 0, nil"), ":\n{}", go);
}

// ---------------------------------------------------------------------------
// Example 03 — worker pool (structs-first, task→go, send/receive, close)
// ---------------------------------------------------------------------------

fn example03_src() -> String {
    r#"
struct job {
    input   int
    cost_ms int
}

struct result {
    worker int
    value  int
}

fn worker(id int, jobs <-chan job, results chan<- result) {
    for j in jobs {
        time.sleep(time.milliseconds(j.cost_ms))
        results <- result{worker: id, value: j.input * 2}
    }
}

fn main() {
    const n_workers = 4
    jobs := make(chan job, 16)
    results := make(chan result, 16)
    for i in 0..n_workers {
        task worker(i, jobs, results)
    }
    inputs := [3, 1, 4, 1, 5, 9, 2, 6]
    for _, v in inputs {
        jobs <- job{input: v, cost_ms: (v % 3)}
    }
    close(jobs)
    var got []int
    for len(got) < len(inputs) {
        r := <-results
        got = append(got, r.value)
    }
    sort.ints(got)
    fmt.println(got)
}
"#
    .to_string()
}

#[test]
fn example03_golden_shape() {
    let go = emit(&example03_src());
    let pkg = go.find("package main").unwrap();
    let imp = go.find("import (").unwrap();
    let ty = go.find("type job struct").unwrap();
    assert!(pkg < imp && imp < ty, "bug-1 shape holds for 03:\n{}", go);
    for p in ["\"fmt\"", "\"sort\"", "\"time\""] {
        assert!(go.contains(p), "auto-import {}:\n{}", p, go);
    }
    assert!(go.contains("func worker(id int, jobs <-chan job, results chan<- result) {"), ":\n{}", go);
    assert!(go.contains("for j := range jobs {"), "1-var range over chan:\n{}", go);
    assert!(
        go.contains("time.Sleep(time.Duration(j.cost_ms) * time.Millisecond)"),
        "duration constructor:\n{}",
        go
    );
    assert!(
        go.contains("results <- result{worker: id, value: j.input * 2}"),
        "struct literal send:\n{}",
        go
    );
    assert!(go.contains("for i := 0; i < n_workers; i++ {"), "range-for:\n{}", go);
    assert!(go.contains("go worker(i, jobs, results)"), "task → go:\n{}", go);
    assert!(
        go.contains("inputs := []int{3, 1, 4, 1, 5, 9, 2, 6}"),
        "inferred int slice:\n{}",
        go
    );
    assert!(go.contains("for _, v := range inputs {"), "underscore range:\n{}", go);
    assert!(go.contains("jobs <- job{input: v, cost_ms: (v % 3)}"), ":\n{}", go);
    assert!(go.contains("close(jobs)"), ":\n{}", go);
    assert!(go.contains("var got []int"), ":\n{}", go);
    assert!(go.contains("r := <-results"), "receive-define:\n{}", go);
    assert!(go.contains("got = append(got, r.value)"), ":\n{}", go);
    assert!(go.contains("sort.Ints(got)"), ":\n{}", go);
    assert!(go.contains("fmt.Println(got)"), ":\n{}", go);
    assert!(go.contains("const n_workers = 4"), "fn-local const:\n{}", go);
}

// ---------------------------------------------------------------------------
// Emission mechanics
// ---------------------------------------------------------------------------

#[test]
fn imports_are_sorted_and_deduped() {
    let go = emit("fn main() {\n    sort.ints(nil)\n    time.sleep(0)\n    fmt.println(\"x\")\n}");
    let imp = go.find("import (").unwrap();
    let end = go.find(")").unwrap();
    let block = &go[imp..end];
    let a = block.find("\"fmt\"").unwrap();
    let s = block.find("\"sort\"").unwrap();
    let t = block.find("\"time\"").unwrap();
    assert!(a < s && s < t, "sorted:\n{}", go);
    assert_eq!(block.matches("\"fmt\"").count(), 1, "deduped:\n{}", go);
}

#[test]
fn interpolation_escapes_percent_signs() {
    let go = emit("fn main() {\n    fmt.println(\"100% done, n={n}\")\n}\n");
    assert!(
        go.contains("fmt.Println(fmt.Sprintf(\"100%% done, n=%v\", n))"),
        "% escaped for Sprintf:\n{}",
        go
    );
}

#[test]
fn all_literal_interp_stays_plain_string() {
    let go = emit("fn main() {\n    fmt.println(\"no braces here\")\n}\n");
    assert!(go.contains("fmt.Println(\"no braces here\")"), ":\n{}", go);
    assert!(!go.contains("Sprintf"), "no Sprintf allocation:\n{}", go);
}

#[test]
fn cooked_strings_re_escape_and_raw_stay_verbatim() {
    let go = emit("fn main() {\n    fmt.println(\"a\\nb\")\n    fmt.println(`raw\\nstring`)\n}");
    assert!(go.contains("\"a\\nb\""), ":\n{}", go);
    assert!(go.contains("`raw\\nstring`"), "raw stays verbatim:\n{}", go);
}

#[test]
fn precedence_needs_no_parens_for_common_mixes() {
    let go = emit("fn main() {\n    x := 1 + 2 * 3\n    y := (1 + 2) * 3\n}");
    assert!(go.contains("x := 1 + 2 * 3"), "no redundant parens:\n{}", go);
    assert!(go.contains("y := (1 + 2) * 3"), "source parens kept:\n{}", go);
}

#[test]
fn comparison_in_call_arg_parses_clean() {
    let go = emit("fn main() {\n    fmt.printf(\"%v\", 1 < 2)\n}");
    assert!(go.contains("fmt.Printf(\"%v\", 1 < 2)"), ":\n{}", go);
}

#[test]
fn while_and_infinite_for() {
    let go = emit("fn main() { for len(x) < 3 { } for { break } }");
    assert!(go.contains("for len(x) < 3 {"), ":\n{}", go);
    assert!(go.contains("for {"), ":\n{}", go);
    assert!(go.contains("break"), ":\n{}", go);
}

#[test]
fn if_else_if_else_chain() {
    let go = emit("fn main() {\n    if a {\n    } else if b {\n    } else {\n    }\n}\n");
    assert!(go.contains("if a {"), ":\n{}", go);
    assert!(go.contains("} else if b {"), "same-line chain:\n{}", go);
    assert!(go.contains("} else {"), ":\n{}", go);
    // Exactly one closing brace per block: no doubled braces.
    assert!(!go.contains("{\n\t\t{\n"), "no nested stray block:\n{}", go);
}

#[test]
fn if_with_init_stmt() {
    let go = emit("fn main() {\n    if v, ok := m[k]; ok {\n    }\n}\n");
    assert!(go.contains("if v, ok := m[k]; ok {"), ":\n{}", go);
}

#[test]
fn send_receive_task_and_move_drop() {
    let go = emit("fn main() {\n    ch <- move agg\n    x := <-ch\n    task worker(x)\n}");
    assert!(go.contains("ch <- agg"), "move marker dropped:\n{}", go);
    assert!(go.contains("x := <-ch"), ":\n{}", go);
    assert!(go.contains("go worker(x)"), ":\n{}", go);
}

#[test]
fn var_and_const_locals() {
    let go = emit("fn main() {\n    var got []int\n    var hits atomic.int64\n    const n = 4\n}");
    assert!(go.contains("var got []int"), ":\n{}", go);
    assert!(go.contains("var hits atomic.Int64"), "dotted foreign type normalized:\n{}", go);
    assert!(go.contains("const n = 4"), ":\n{}", go);
}

#[test]
fn named_results_on_plain_fn() {
    let go = emit("fn f() (code int) {\n    return 3\n}\n");
    assert!(go.contains("func f() (code int) {"), ":\n{}", go);
    assert!(go.contains("return 3"), ":\n{}", go);
}

#[test]
fn method_receiver_and_struct_methods() {
    let go = emit("struct g {\n    n int\n}\n\nfn (p *g) add(x int) {\n    p.n += x\n}\n");
    assert!(go.contains("func (p *g) add(x int) {"), ":\n{}", go);
}

#[test]
fn addr_of_and_index_inc() {
    // (`*p` deref is not in the parser's MVP surface — pointer structs use
    // auto-deref field access, exactly like Go.)
    let go = emit("fn main() {\n    p := &x\n    m[k]++\n}");
    assert!(go.contains("p := &x"), ":\n{}", go);
    assert!(go.contains("m[k]++"), ":\n{}", go);
}

#[test]
fn multi_define_from_two_value_call() {
    let go = emit("fn main() {\n    a, b := f()\n}");
    assert!(go.contains("a, b := f()"), ":\n{}", go);
}

#[test]
fn struct_literal_fields_keep_our_names() {
    let go = emit("struct job {\n    cost_ms int\n}\n\nfn main() {\n    j := job{cost_ms: 3}\n}\n");
    assert!(go.contains("job{cost_ms: 3}"), "our fields NOT camelized:\n{}", go);
}

#[test]
fn slice_expr_forms() {
    let go = emit("fn main() {\n    a := xs[1:]\n    b := xs[:2]\n    c := xs[1:2]\n}");
    assert!(go.contains("a := xs[1:]"), ":\n{}", go);
    assert!(go.contains("b := xs[:2]"), ":\n{}", go);
    assert!(go.contains("c := xs[1:2]"), ":\n{}", go);
}

#[test]
fn spread_argument() {
    let go = emit("fn main() {\n    f(xs...)\n}");
    assert!(go.contains("f(xs...)"), ":\n{}", go);
}

#[test]
fn managed_block_emits_marker_comment() {
    let go = emit("fn main() {\n    managed {\n        task w(counter)\n    }\n}\n");
    assert!(go.contains("// managed"), "audit marker greppable:\n{}", go);
    assert!(go.contains("go w(counter)"), ":\n{}", go);
}

#[test]
fn main_without_results_or_try_stays_plain() {
    let go = emit("fn main() {\n    fmt.println(\"x\")\n}\n");
    assert!(go.contains("func main() {"), ":\n{}", go);
    assert!(!go.contains("func() ("), "no wrapper closure:\n{}", go);
    assert!(!go.contains("os.Exit"), "no exit plumbing:\n{}", go);
}

#[test]
fn main_with_try_but_no_results_gets_wrapper() {
    let go = emit("fn main() {\n    f := os.open(p)?\n}\n");
    assert!(go.contains("func main() {"), ":\n{}", go);
    assert!(go.contains("func() (hyErr error) {"), "error-only closure:\n{}", go);
    assert!(go.contains("os.Exit(1)"), ":\n{}", go);
    assert!(!go.contains("os.Exit(code)"), "no code var:\n{}", go);
}

// ---------------------------------------------------------------------------
// Clean unsupported errors — never a wrong emission
// ---------------------------------------------------------------------------

#[test]
fn rust_crate_import_errors_cleanly() {
    let msg = emit_err("import \"rust/serde_json\" fn main() { }");
    assert!(msg.contains("bridge"), "points at the Phase-2 bridge: {}", msg);
}

#[test]
fn enum_decl_errors_cleanly() {
    let msg = emit_err("enum shape { circle square } fn main() { }");
    assert!(msg.contains("enum"), "{}", msg);
}

#[test]
fn match_stmt_errors_cleanly() {
    let msg = emit_err("fn main() { match x { _ => { } } }");
    assert!(msg.contains("match"), "{}", msg);
}

#[test]
fn select_stmt_errors_cleanly() {
    let msg = emit_err("fn main() { select { } }");
    assert!(msg.contains("select"), "{}", msg);
}

#[test]
fn type_elided_struct_lit_errors_cleanly() {
    let msg = emit_err("fn main() { append(rows, {name: 1}) }");
    assert!(msg.contains("type-elided"), "{}", msg);
}

#[test]
fn try_outside_binding_position_errors_cleanly() {
    let msg = emit_err("fn main() { fmt.println(f()?) }");
    assert!(msg.contains("`?`"), "{}", msg);
}
