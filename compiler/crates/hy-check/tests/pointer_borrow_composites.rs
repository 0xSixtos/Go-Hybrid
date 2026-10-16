use hy_check::{check_file, Verdict};
use hy_syntax::parse_file;

fn check(src: &str) -> hy_check::CheckResult {
    let path = "inline.hy";
    let ast = parse_file(path, src).unwrap_or_else(|diags| panic!("must parse: {:?}", diags));
    check_file(&ast)
}

#[test]
fn negative_regression_direct_slice_move_freezes_referenced_local() {
    let src = r#"
struct node {
    val int
}

fn worker(s []*node, done chan int) {
    s[0].val = 999
    done <- 1
}

fn main() {
    n := node{val: 0}
    s := []*node{&n}
    done := make(chan int, 1)
    task worker(move s, done)
    n.val = 1
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn negative_regression_param_pointer_slice_move_is_e3004() {
    let src = r#"
struct node {
    val int
}

fn worker(s []*node, done chan int) {
    s[0].val = 999
    done <- 1
}

fn run(n node, done chan int) {
    s := []*node{&n}
    task worker(move s, done)
}

fn main() {
    n := node{val: 0}
    done := make(chan int, 1)
    run(n, done)
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3004"));
}

#[test]
fn negative_regression_const_mutation_through_pointer_slice_is_e3006() {
    let src = r#"
struct node {
    val int
}

const global_node = node{val: 0}

fn main() {
    s := []*node{&global_node}
    s[0].val = 999
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3006"));
}

#[test]
fn negative_regression_struct_field_holding_pointer_slice_is_e3103() {
    let src = r#"
struct node {
    val int
}

struct box_holder {
    nodes []*node
}

fn main() {
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3103"));
}

#[test]
fn negative_regression_struct_field_holding_borrow_handle_slice_is_e3103() {
    let src = r#"
struct box_holder {
    views []serde_json.view
}

fn main() {
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3103"));
}

#[test]
fn positive_control_owned_move_without_post_use_accepts() {
    let src = r#"
struct node {
    val int
}

fn worker(s []*node, done chan int) {
    s[0].val = 999
    done <- 1
}

fn main() {
    n := node{val: 0}
    s := []*node{&n}
    done := make(chan int, 1)
    task worker(move s, done)
    <-done
}
"#;
    let res = check(src);
    assert_eq!(res.diagnostics.len(), 0);
    assert!(matches!(res.verdict, Verdict::Accept));
}

#[test]
fn positive_control_scalar_atomic_pointer_field_accepts() {
    let src = r#"
struct box_holder {
    p *atomic.int64
}

fn main() {
}
"#;
    let res = check(src);
    assert_eq!(res.diagnostics.len(), 0);
    assert!(matches!(res.verdict, Verdict::Accept));
}

#[test]
fn edge_case_map_of_pointers() {
    let src = r#"
struct node {
    val int
}

fn worker(m map[string]*node, done chan int) {
    m["k"].val = 999
    done <- 1
}

fn main() {
    n := node{val: 0}
    m := map[string]*node{"k": &n}
    done := make(chan int, 1)
    task worker(move m, done)
    n.val = 1
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn edge_case_array_of_pointers() {
    let src = r#"
struct node {
    val int
}

fn worker(a []*node, done chan int) {
    a[0].val = 999
    done <- 1
}

fn main() {
    n := node{val: 0}
    a := [&n]
    done := make(chan int, 1)
    task worker(move a, done)
    n.val = 1
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn edge_case_chan_send_move_freezes_referenced_local() {
    let src = r#"
struct node {
    val int
}

fn worker(ch chan []*node, done chan int) {
    s := <-ch
    s[0].val = 999
    done <- 1
}

fn main() {
    n := node{val: 0}
    s := []*node{&n}
    ch := make(chan []*node, 1)
    done := make(chan int, 1)
    task worker(ch, done)
    ch <- move s
    n.val = 1
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn multiple_borrow_roots_in_slice_freezes_first_root() {
    let src = r#"
struct node {
    val int
}

fn worker(s []*node, done chan int) {
    s[0].val = 999
    done <- 1
}

fn main() {
    n1 := node{val: 0}
    n2 := node{val: 0}
    s := []*node{&n1, &n2}
    done := make(chan int, 1)
    task worker(move s, done)
    n1.val = 1
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn multiple_borrow_roots_in_slice_freezes_second_root() {
    let src = r#"
struct node {
    val int
}

fn worker(s []*node, done chan int) {
    s[1].val = 999
    done <- 1
}

fn main() {
    n1 := node{val: 0}
    n2 := node{val: 0}
    s := []*node{&n1, &n2}
    done := make(chan int, 1)
    task worker(move s, done)
    n2.val = 2
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn deeply_nested_slice_of_slices_freezes_all_roots() {
    let src = r#"
struct node {
    val int
}

fn worker_grid(grid [][]*node, done chan int) {
    grid[0][0].val = 999
    done <- 1
}

fn main() {
    n1 := node{val: 0}
    n2 := node{val: 0}
    grid := [][]*node{ []*node{&n1}, []*node{&n2} }
    done := make(chan int, 1)
    task worker_grid(move grid, done)
    n1.val = 1
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn deeply_nested_slice_of_slices_freezes_second_root() {
    let src = r#"
struct node {
    val int
}

fn worker_grid(grid [][]*node, done chan int) {
    grid[1][0].val = 999
    done <- 1
}

fn main() {
    n1 := node{val: 0}
    n2 := node{val: 0}
    grid := [][]*node{ []*node{&n1}, []*node{&n2} }
    done := make(chan int, 1)
    task worker_grid(move grid, done)
    n2.val = 2
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn deeply_nested_map_of_pointer_slices_freezes_root() {
    let src = r#"
struct node {
    val int
}

fn worker_map(m map[string][]*node, done chan int) {
    m["row"][0].val = 999
    done <- 1
}

fn main() {
    n := node{val: 0}
    m := map[string][]*node{"row": []*node{&n}}
    done := make(chan int, 1)
    task worker_map(move m, done)
    n.val = 1
    <-done
}
"#;
    let res = check(src);
    assert!(matches!(res.verdict, Verdict::Reject));
    assert!(res.diagnostics.iter().any(|d| d.code == "HYB-E3003"));
}

#[test]
fn positive_control_multiple_borrow_roots_unaccessed_post_move() {
    let src = r#"
struct node {
    val int
}

fn worker(s []*node, done chan int) {
    s[0].val = 999
    done <- 1
}

fn main() {
    n1 := node{val: 0}
    n2 := node{val: 0}
    s := []*node{&n1, &n2}
    done := make(chan int, 1)
    task worker(move s, done)
    <-done
}
"#;
    let res = check(src);
    assert_eq!(res.diagnostics.len(), 0);
    assert!(matches!(res.verdict, Verdict::Accept));
}

#[test]
fn positive_control_deeply_nested_composites_unaccessed_post_move() {
    let src = r#"
struct node {
    val int
}

fn worker_grid(grid [][]*node, done chan int) {
    grid[0][0].val = 999
    done <- 1
}

fn main() {
    n1 := node{val: 0}
    n2 := node{val: 0}
    grid := [][]*node{ []*node{&n1}, []*node{&n2} }
    done := make(chan int, 1)
    task worker_grid(move grid, done)
    <-done
}
"#;
    let res = check(src);
    assert_eq!(res.diagnostics.len(), 0);
    assert!(matches!(res.verdict, Verdict::Accept));
}
