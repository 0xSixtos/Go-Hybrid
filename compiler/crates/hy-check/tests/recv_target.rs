//! Regression for issue #409: receive-operand channel-ness checks
//! (HYB-E2008 concrete non-channel / HYB-E3005 fail-closed unknown).
//! `<-x` on a non-channel used to pass with 0 diagnostics and emit Go
//! that fails `go build` (`cannot receive from non-channel`) — the
//! receive-side mirror of the send rule (#77, HYB-E3007).

use hy_check::{check_file, Verdict};
use hy_diag::Severity;
use hy_syntax::parse_file;

fn check(src: &str) -> hy_check::CheckResult {
    let path = "inline.hy";
    let ast = parse_file(path, src)
        .unwrap_or_else(|diags| panic!("must parse (parser contract): {:?}", diags));
    check_file(&ast)
}

fn error_codes(r: &hy_check::CheckResult) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.clone())
        .collect()
}

fn assert_has(r: &hy_check::CheckResult, code: &str) {
    let cs = error_codes(r);
    assert!(
        cs.iter().any(|c| c == code),
        "expected {}, got {:?}",
        code,
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

fn assert_accepts(src: &str) {
    let r = check(src);
    let errs = error_codes(&r);
    assert!(
        errs.is_empty(),
        "expected clean accept, got {:?}\nsrc: {}",
        errs,
        src
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

// ---- R1-R4 from the issue: the everyday non-channel receive shapes ----

#[test]
fn recv_from_int_literal_rejected() {
    let r = check("fn main() {\n    <-1\n}\n");
    assert_has(&r, "HYB-E2008");
}

#[test]
fn recv_from_int_binding_rejected() {
    let r = check("fn main() {\n    x := 5\n    <-x\n}\n");
    assert_has(&r, "HYB-E2008");
}

#[test]
fn recv_from_int_call_result_rejected() {
    let r = check("fn five() int {\n    return 5\n}\n\nfn main() {\n    <-five()\n}\n");
    assert_has(&r, "HYB-E2008");
}

#[test]
fn recv_from_map_rejected() {
    let r = check("fn main() {\n    m := map[string]int{}\n    <-m\n}\n");
    assert_has(&r, "HYB-E2008");
}

// ---- binding form ----

#[test]
fn recv_into_binding_from_non_channel_rejected() {
    let r = check("fn main() {\n    x := 5\n    v := <-x\n    _ = v\n}\n");
    assert_has(&r, "HYB-E2008");
}

// ---- nesting: the operand check fires at expression depth ----

#[test]
fn recv_nested_in_call_args_rejected() {
    let r = check(
        "fn sink(n int) {\n}\n\nfn main() {\n    x := 5\n    sink(<-x)\n}\n",
    );
    assert_has(&r, "HYB-E2008");
}

#[test]
fn recv_nested_in_binary_rejected() {
    let r = check("fn main() {\n    x := 5\n    _ = 1 + <-x\n}\n");
    assert_has(&r, "HYB-E2008");
}

#[test]
fn recv_nested_in_send_value_rejected() {
    // `ch <- <-x`: the send target is a channel, but the nested receive
    // source is not — the nested operand must still be classified.
    let r = check("fn main() {\n    ch := make(chan int, 1)\n    x := 5\n    ch <- <-x\n}\n");
    assert_has(&r, "HYB-E2008");
}

// ---- select arm ----

#[test]
fn select_recv_arm_from_non_channel_rejected() {
    let r = check(
        "fn main() {\n    x := 5\n    select {\n    v := <-x:\n        _ = v\n    default:\n    }\n}\n",
    );
    assert_has(&r, "HYB-E2008");
}

// ---- closure body: outside the Tier-0 flat walk (A5 perimeter) ----

#[test]
fn recv_in_closure_body_rejected() {
    let r = check(
        "fn main() {\n    x := 5\n    f := fn() {\n        <-x\n    }\n    f()\n}\n",
    );
    assert_has(&r, "HYB-E2008");
}

#[test]
fn recv_in_nested_closure_rejected() {
    let r = check(
        "fn main() {\n    x := 5\n    outer := fn() {\n        inner := fn() {\n            <-x\n        }\n        inner()\n    }\n    outer()\n}\n",
    );
    assert_has(&r, "HYB-E2008");
}

#[test]
fn recv_in_managed_block_rejected() {
    let r = check(
        "fn main() {\n    x := 5\n    managed(\"reason\", \"mutex\") {\n        <-x\n    }\n}\n",
    );
    assert_has(&r, "HYB-E2008");
}

// ---- positive controls: legitimate receives keep passing ----

#[test]
fn recv_from_chan_binding_accepted() {
    assert_accepts("fn main() {\n    ch := make(chan int, 1)\n    ch <- 7\n    v := <-ch\n    _ = v\n}\n");
}

#[test]
fn bare_recv_from_chan_binding_accepted() {
    assert_accepts("fn main() {\n    ch := make(chan int, 1)\n    ch <- 7\n    <-ch\n}\n");
}

#[test]
fn recv_from_chan_param_accepted() {
    assert_accepts(
        "fn work(jobs chan int, done chan bool) {\n    for {\n        j := <-jobs\n        _ = j\n        done <- true\n        break\n    }\n}\n",
    );
}

#[test]
fn recv_in_closure_from_captured_chan_accepted() {
    assert_accepts(
        "fn main() {\n    ch := make(chan int, 1)\n    ch <- 9\n    f := fn() {\n        v := <-ch\n        _ = v\n    }\n    f()\n}\n",
    );
}

#[test]
fn select_recv_arm_from_channel_accepted() {
    assert_accepts(
        "fn main() {\n    ch := make(chan int, 1)\n    ch <- 3\n    select {\n    v := <-ch:\n        _ = v\n    default:\n    }\n}\n",
    );
}
