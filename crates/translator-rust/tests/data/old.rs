pub fn pick(flag: bool, ok: bool, v: &[i64]) -> i64 {
    if flag {
        panic!("flag set");
    }
    if ok { v[0] } else { 0 }
}
