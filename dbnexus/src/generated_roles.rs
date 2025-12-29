//! 生成的权限角色模块
//!
//! 此模块由 build.rs 自动生成，包含 permissions.yaml 中定义的所有角色
//! 用于 #[db_permission] 宏的编译时验证

include!(concat!(env!("OUT_DIR"), "/generated_roles.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-019: 生成角色验证测试
    #[test]
    fn test_generated_roles_basic() {
        // 验证函数存在且可调用
        // 如果没有定义角色，返回空数组
        let defined_roles = get_defined_roles();
        assert!(!defined_roles.contains(&"nonexistent_role"));
    }
}
