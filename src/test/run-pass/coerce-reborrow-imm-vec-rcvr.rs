// xfail-test

fn foo(v: &[const uint]) -> ~[uint] {
    v.to_vec()
}

fn bar(v: &mut [uint]) -> ~[uint] {
    v.to_vec()
}

fn bip(v: &[uint]) -> ~[uint] {
    v.to_vec()
}

pub fn main() {
    let mut the_vec = ~[1, 2, 3, 100];
    fail_unless!(the_vec == foo(the_vec));
    fail_unless!(the_vec == bar(the_vec));
    fail_unless!(the_vec == bip(the_vec));
}
