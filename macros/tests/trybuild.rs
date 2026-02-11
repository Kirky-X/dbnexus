use trybuild::TestCases;

#[test]
fn test_db_crud_table_name_arg() {
    let t = TestCases::new();
    t.pass("tests/ui/db_crud_table_name_arg.rs");
}
