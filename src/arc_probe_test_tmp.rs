#[test]
fn arc_probe_strong_count() {
    use std::sync::Arc;
    let a = Arc::new(42u32);
    let held = a.clone();
    assert_eq!(Arc::strong_count(&a), 2);
    let r = Arc::try_unwrap(a);
    assert!(r.is_err(), "2 refs must fail try_unwrap");
    drop(held);
}
