// db_permission 宏属性参数过长错误测试
use dbnexus::DbEntity;
use dbnexus::db_permission;

#[derive(DbEntity, Clone, Debug)]
#[table_name = "users"]
#[db_permission(roles = "very_long_role_name_that_exceeds_the_maximum_allowed_length_limit_for_role_names_in_this_system_which_is_set_to_prevent_potential_denial_of_service_attacks_or_buffer_overflow_issues_in_the_macro_processing_code")]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

fn main() {}
