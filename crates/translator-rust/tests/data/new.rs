pub fn pick(flag: bool, ok: bool, v: &[i64]) -> i64 {
    if flag {
        return 0;
    }
    if ok { v[0] } else { 0 }
}
