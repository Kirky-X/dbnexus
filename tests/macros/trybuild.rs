use trybuild::TestCases;

#[test]
fn db_entity_pass_tests() {
    let t = TestCases::new();
    t.pass("tests/ui/db_entity_basic.rs");
    t.pass("tests/ui/db_entity_with_permissions.rs");
    t.pass("tests/ui/db_entity_with_cache.rs");
    t.pass("tests/ui/db_entity_with_audit.rs");
}

#[test]
fn db_entity_compile_fail_tests() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/db_entity_missing_table_name.rs");
    t.compile_fail("tests/ui/db_entity_missing_primary_key.rs");
}
