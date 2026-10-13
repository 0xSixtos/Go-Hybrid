fn main() {
    let src = std::fs::read_to_string("/tmp/dump07.hy").unwrap();
    let f = hy_syntax::parse_file("t.hy", &src).unwrap();
    println!("{}", hy_syntax::dump_ast(&f));
}
