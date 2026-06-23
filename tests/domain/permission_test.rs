// Domain Permission Module Tests

#[cfg(feature = "permission")]
mod permission_tests {
    use dbnexus::domain::permission::{
        PermissionAction, PermissionConfig, PermissionProvider, RolePolicy, TablePermission,
    };
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_memory_permission_provider_check_admin() {
        let provider = dbnexus::domain::permission::new_in_memory();

        let result = provider.check("admin", "users", PermissionAction::Select).await;
        assert!(result.is_ok());
        assert!(result.unwrap()); // admin should always be allowed
    }

    #[tokio::test]
    async fn test_memory_permission_provider_check_missing_role() {
        let provider = dbnexus::domain::permission::new_in_memory();

        let result = provider.check("unknown_role", "users", PermissionAction::Select).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_role_policy_allows() {
        let mut policy = RolePolicy::default();
        policy.tables.push(TablePermission {
            name: "users".to_string(),
            operations: vec![PermissionAction::Select, PermissionAction::Insert],
        });

        assert!(policy.allows("users", &PermissionAction::Select));
        assert!(policy.allows("users", &PermissionAction::Insert));
        assert!(!policy.allows("users", &PermissionAction::Delete));
        assert!(!policy.allows("orders", &PermissionAction::Select));
    }

    #[test]
    fn test_role_policy_wildcard() {
        let mut policy = RolePolicy::default();
        policy.tables.push(TablePermission {
            name: "*".to_string(),
            operations: vec![PermissionAction::Select],
        });

        assert!(policy.allows("users", &PermissionAction::Select));
        assert!(policy.allows("orders", &PermissionAction::Select));
        assert!(!policy.allows("users", &PermissionAction::Insert));
    }
}
