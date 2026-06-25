// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License.
// See LICENSE file in the project root for full license information.

//! Domain Permission 模块单元测试
//!
//! 覆盖范围：
//! - `PermissionConfig` 构造、默认值、语义校验
//! - `PermissionConfigError` 变体
//! - `DefaultPolicy` 序列化/默认值
//! - `PermissionAction` 枚举与 Display
//! - `TablePermission::allows`
//! - `RolePolicy::allows`（含通配符）
//! - `PolicySet` 聚合
//! - `MemoryPermissionProvider`：check / get_policy / refresh / health_check / shutdown
//! - `YamlPermissionProvider`：从 YAML 字符串加载、check（admin / 命中 / 默认策略）

#[cfg(feature = "permission")]
mod permission_tests {
    use dbnexus::domain::permission::{
        new, new_in_memory, DefaultPolicy, PermissionAction, PermissionChecker, PermissionConfig,
        PermissionConfigError, PermissionError, PermissionLifecycle, PermissionProvider,
        PolicyManager, PolicySet, RolePolicy, TablePermission,
    };
    use std::collections::HashMap;
    use std::sync::Arc;

    // =========================================================================
    // PermissionAction
    // =========================================================================

    #[test]
    fn test_permission_action_display() {
        assert_eq!(PermissionAction::Select.to_string(), "SELECT");
        assert_eq!(PermissionAction::Insert.to_string(), "INSERT");
        assert_eq!(PermissionAction::Update.to_string(), "UPDATE");
        assert_eq!(PermissionAction::Delete.to_string(), "DELETE");
    }

    #[test]
    fn test_permission_action_equality() {
        assert_eq!(PermissionAction::Select, PermissionAction::Select);
        assert_ne!(PermissionAction::Select, PermissionAction::Insert);
    }

    #[test]
    fn test_permission_action_serde_roundtrip() {
        let json = serde_json::to_string(&PermissionAction::Delete).unwrap();
        assert_eq!(json, "\"Delete\"");
        let parsed: PermissionAction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PermissionAction::Delete);
    }

    // =========================================================================
    // TablePermission
    // =========================================================================

    #[test]
    fn test_table_permission_allows() {
        let tp = TablePermission {
            name: "users".into(),
            operations: vec![PermissionAction::Select, PermissionAction::Insert],
        };
        assert!(tp.allows(&PermissionAction::Select));
        assert!(tp.allows(&PermissionAction::Insert));
        assert!(!tp.allows(&PermissionAction::Update));
        assert!(!tp.allows(&PermissionAction::Delete));
    }

    #[test]
    fn test_table_permission_empty_operations() {
        let tp = TablePermission {
            name: "users".into(),
            operations: vec![],
        };
        for action in [
            PermissionAction::Select,
            PermissionAction::Insert,
            PermissionAction::Update,
            PermissionAction::Delete,
        ] {
            assert!(!tp.allows(&action));
        }
    }

    // =========================================================================
    // RolePolicy
    // =========================================================================

    #[test]
    fn test_role_policy_allows_specific_table() {
        let mut policy = RolePolicy::default();
        policy.tables.push(TablePermission {
            name: "users".to_string(),
            operations: vec![PermissionAction::Select, PermissionAction::Insert],
        });

        assert!(policy.allows("users", &PermissionAction::Select));
        assert!(policy.allows("users", &PermissionAction::Insert));
        assert!(!policy.allows("users", &PermissionAction::Delete));
        // 未配置的表应拒绝
        assert!(!policy.allows("orders", &PermissionAction::Select));
    }

    #[test]
    fn test_role_policy_wildcard_table() {
        let mut policy = RolePolicy::default();
        policy.tables.push(TablePermission {
            name: "*".to_string(),
            operations: vec![PermissionAction::Select],
        });

        // 通配符匹配任意表名
        assert!(policy.allows("users", &PermissionAction::Select));
        assert!(policy.allows("orders", &PermissionAction::Select));
        // 通配符不扩展到未列出的操作
        assert!(!policy.allows("users", &PermissionAction::Insert));
    }

    #[test]
    fn test_role_policy_multiple_tables() {
        let mut policy = RolePolicy::default();
        policy.tables.push(TablePermission {
            name: "users".to_string(),
            operations: vec![PermissionAction::Select],
        });
        policy.tables.push(TablePermission {
            name: "orders".to_string(),
            operations: vec![PermissionAction::Insert, PermissionAction::Update],
        });

        assert!(policy.allows("users", &PermissionAction::Select));
        assert!(policy.allows("orders", &PermissionAction::Insert));
        assert!(policy.allows("orders", &PermissionAction::Update));
        assert!(!policy.allows("users", &PermissionAction::Insert));
        assert!(!policy.allows("orders", &PermissionAction::Select));
    }

    #[test]
    fn test_role_policy_empty() {
        let policy = RolePolicy::default();
        // 空策略应拒绝所有操作
        assert!(!policy.allows("users", &PermissionAction::Select));
    }

    #[test]
    fn test_role_policy_serde_roundtrip() {
        let mut policy = RolePolicy::default();
        policy.tables.push(TablePermission {
            name: "users".to_string(),
            operations: vec![PermissionAction::Select, PermissionAction::Delete],
        });
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: RolePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tables.len(), 1);
        assert_eq!(parsed.tables[0].name, "users");
        assert_eq!(parsed.tables[0].operations.len(), 2);
    }

    // =========================================================================
    // PolicySet
    // =========================================================================

    #[test]
    fn test_policy_set_default() {
        let set = PolicySet::default();
        assert!(set.roles.is_empty());
    }

    #[test]
    fn test_policy_set_with_roles() {
        let mut set = PolicySet::default();
        let mut admin_policy = RolePolicy::default();
        admin_policy.tables.push(TablePermission {
            name: "*".to_string(),
            operations: vec![
                PermissionAction::Select,
                PermissionAction::Insert,
                PermissionAction::Update,
                PermissionAction::Delete,
            ],
        });
        set.roles.insert("admin".into(), admin_policy);

        assert_eq!(set.roles.len(), 1);
        assert!(set.roles.contains_key("admin"));
    }

    // =========================================================================
    // DefaultPolicy
    // =========================================================================
    // 注意：DefaultPolicy 仅 derive(Debug, Clone, Copy, Default, Deserialize)，
    // 未实现 PartialEq / Serialize，因此这里只能用 matches! 和反序列化测试。

    #[test]
    fn test_default_policy_default_is_deny_all() {
        let dp = DefaultPolicy::default();
        assert!(matches!(dp, DefaultPolicy::DenyAll));
    }

    #[test]
    fn test_default_policy_deserialize_snake_case() {
        // serde rename_all = "snake_case"
        let parsed: DefaultPolicy = serde_json::from_str("\"allow_all\"").unwrap();
        assert!(matches!(parsed, DefaultPolicy::AllowAll));
        let parsed: DefaultPolicy = serde_json::from_str("\"deny_all\"").unwrap();
        assert!(matches!(parsed, DefaultPolicy::DenyAll));
    }

    // =========================================================================
    // PermissionConfig
    // =========================================================================

    #[test]
    fn test_permission_config_default() {
        let cfg = PermissionConfig::default();
        assert_eq!(cfg.admin_role, "admin");
        assert!(matches!(cfg.default_policy, DefaultPolicy::DenyAll));
        assert!(!cfg.rate_limit_enabled);
        assert_eq!(cfg.rate_limit_max_requests, 100);
        assert!(cfg.policy_path.is_none());
    }

    #[test]
    fn test_permission_config_validate_ok() {
        let cfg = PermissionConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_permission_config_validate_empty_admin_role() {
        let mut cfg = PermissionConfig::default();
        cfg.admin_role = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(
            matches!(err, PermissionConfigError::MissingField(ref f) if f == "admin_role"),
            "expected MissingField(\"admin_role\"), got {err:?}"
        );
    }

    #[test]
    fn test_permission_config_validate_rate_limit_zero_when_enabled() {
        let mut cfg = PermissionConfig::default();
        cfg.rate_limit_enabled = true;
        cfg.rate_limit_max_requests = 0;
        let err = cfg.validate().unwrap_err();
        assert!(
            matches!(err, PermissionConfigError::InvalidValue { ref field, .. } if field == "rate_limit_max_requests"),
            "expected InvalidValue for rate_limit_max_requests, got {err:?}"
        );
    }

    #[test]
    fn test_permission_config_validate_rate_limit_ok_when_disabled() {
        // rate_limit_max_requests == 0 但 rate_limit_enabled = false 应通过
        let mut cfg = PermissionConfig::default();
        cfg.rate_limit_enabled = false;
        cfg.rate_limit_max_requests = 0;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_permission_config_deserialize_from_yaml() {
        let yaml = r#"
admin_role: root
default_policy: allow_all
rate_limit_enabled: true
rate_limit_max_requests: 50
"#;
        let cfg: PermissionConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(cfg.admin_role, "root");
        assert!(matches!(cfg.default_policy, DefaultPolicy::AllowAll));
        assert!(cfg.rate_limit_enabled);
        assert_eq!(cfg.rate_limit_max_requests, 50);
    }

    // =========================================================================
    // PermissionConfigError
    // =========================================================================

    #[test]
    fn test_permission_config_error_display_missing_field() {
        let err = PermissionConfigError::MissingField("foo".into());
        let msg = err.to_string();
        assert!(msg.contains("missing required field"));
        assert!(msg.contains("foo"));
    }

    #[test]
    fn test_permission_config_error_display_invalid_value() {
        let err = PermissionConfigError::InvalidValue {
            field: "bar".into(),
            reason: "must be positive".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("bar"));
        assert!(msg.contains("must be positive"));
    }

    #[test]
    fn test_permission_config_error_display_policy_file_not_found() {
        let err = PermissionConfigError::PolicyFileNotFound("/tmp/missing.yaml".into());
        let msg = err.to_string();
        assert!(msg.contains("policy file not found"));
        assert!(msg.contains("/tmp/missing.yaml"));
    }

    // =========================================================================
    // PermissionError
    // =========================================================================

    #[test]
    fn test_permission_error_display_denied() {
        let err = PermissionError::Denied {
            resource: "users".into(),
            operation: "SELECT".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("permission denied"));
        assert!(msg.contains("users"));
        assert!(msg.contains("SELECT"));
    }

    #[test]
    fn test_permission_error_display_role_not_found() {
        let err = PermissionError::RoleNotFound("guest".into());
        assert!(err.to_string().contains("role not found: guest"));
    }

    #[test]
    fn test_permission_error_display_invalid_policy() {
        let err = PermissionError::InvalidPolicy("bad config".into());
        assert!(err.to_string().contains("bad config"));
    }

    #[test]
    fn test_permission_error_display_rate_limited() {
        let err = PermissionError::RateLimited;
        assert!(err.to_string().contains("rate limit exceeded"));
    }

    #[test]
    fn test_permission_error_display_parse_error() {
        let err = PermissionError::ParseError("yaml broken".into());
        assert!(err.to_string().contains("yaml broken"));
    }

    // =========================================================================
    // MemoryPermissionProvider
    // =========================================================================

    #[tokio::test]
    async fn test_memory_permission_provider_check_admin() {
        let provider = new_in_memory();
        let result: Result<bool, PermissionError> =
            provider.check("admin", "users", PermissionAction::Select).await;
        assert!(result.is_ok());
        assert!(result.unwrap(), "admin should always be allowed");
    }

    #[tokio::test]
    async fn test_memory_permission_provider_check_unknown_role() {
        let provider = new_in_memory();
        let result = provider
            .check("unknown_role", "users", PermissionAction::Select)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, PermissionError::RoleNotFound(ref r) if r == "unknown_role"),
            "expected RoleNotFound(\"unknown_role\"), got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_memory_permission_provider_get_policy_empty() {
        let provider = new_in_memory();
        let result = provider.get_policy("admin").await;
        assert!(result.is_ok());
        // MemoryPermissionProvider 初始化时不包含任何策略
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_memory_permission_provider_refresh_no_op() {
        let provider = new_in_memory();
        // 内存实现的 refresh 是 no-op，应始终成功
        assert!(provider.refresh().await.is_ok());
    }

    #[tokio::test]
    async fn test_memory_permission_provider_health_check() {
        let provider = new_in_memory();
        assert!(provider.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_memory_permission_provider_shutdown_clears_policies() {
        let provider = new_in_memory();
        // shutdown 不应 panic
        provider.shutdown().await;
        // shutdown 后仍可访问（admin 仍允许）
        let result = provider.check("admin", "users", PermissionAction::Select).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    // =========================================================================
    // YamlPermissionProvider (通过 new() 工厂函数)
    // =========================================================================

    #[tokio::test]
    async fn test_yaml_provider_new_without_policy_path() {
        // 无 policy_path 时不应尝试加载文件
        let cfg = PermissionConfig::default();
        let provider = new(cfg).await;
        assert!(provider.is_ok(), "expected Ok, got {:?}", provider.err());
    }

    #[tokio::test]
    async fn test_yaml_provider_new_with_invalid_path() {
        let cfg = PermissionConfig {
            policy_path: Some("/nonexistent/path/policy.yaml".into()),
            ..PermissionConfig::default()
        };
        let result = new(cfg).await;
        // 应返回 InvalidValue 错误（包装 PermissionError::ParseError）
        // 注意：impl PermissionProvider 未实现 Debug，不能用 unwrap_err()
        match result {
            Err(PermissionConfigError::InvalidValue { ref field, .. }) if field == "policy_path" => {}
            Err(other) => panic!("expected InvalidValue for policy_path, got {other:?}"),
            Ok(_) => panic!("expected error for nonexistent policy path"),
        }
    }

    #[tokio::test]
    async fn test_yaml_provider_new_empty_admin_role_fails_validation() {
        let cfg = PermissionConfig {
            admin_role: String::new(),
            ..PermissionConfig::default()
        };
        let result = new(cfg).await;
        match result {
            Err(PermissionConfigError::MissingField(ref f)) if f == "admin_role" => {}
            Err(other) => panic!("expected MissingField(\"admin_role\"), got {other:?}"),
            Ok(_) => panic!("expected error for empty admin_role"),
        }
    }

    // =========================================================================
    // YamlPermissionProvider - 从临时 YAML 文件加载策略
    // =========================================================================

    #[tokio::test]
    async fn test_yaml_provider_load_from_file_and_check() {
        // 写入临时 YAML 策略文件
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let path = dir.path().join("policy.yaml");
        let yaml = r#"
admin:
  tables:
    - name: "*"
      operations:
        - Select
        - Insert
        - Update
        - Delete
reader:
  tables:
    - name: "users"
      operations:
        - Select
"#;
        std::fs::write(&path, yaml).expect("failed to write policy.yaml");

        let cfg = PermissionConfig {
            policy_path: Some(path.to_string_lossy().into_owned()),
            ..PermissionConfig::default()
        };
        let provider = new(cfg).await.expect("provider creation failed");

        // admin 始终允许（由 admin_role 配置决定，非策略文件中的 admin 条目）
        assert!(provider.check("admin", "users", PermissionAction::Delete).await.unwrap());

        // reader 对 users 表有 Select 权限
        assert!(provider.check("reader", "users", PermissionAction::Select).await.unwrap());
        // reader 对 users 表没有 Delete 权限
        assert!(!provider.check("reader", "users", PermissionAction::Delete).await.unwrap());
        // reader 对未配置的 orders 表应按 DenyAll 策略拒绝
        assert!(!provider.check("reader", "orders", PermissionAction::Select).await.unwrap());

        // 清理
        drop(provider);
        drop(dir);
    }

    #[tokio::test]
    async fn test_yaml_provider_default_policy_allow_all() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let path = dir.path().join("policy.yaml");
        // 空策略文件
        std::fs::write(&path, "{}").expect("failed to write policy.yaml");

        let cfg = PermissionConfig {
            policy_path: Some(path.to_string_lossy().into_owned()),
            default_policy: DefaultPolicy::AllowAll,
            ..PermissionConfig::default()
        };
        let provider = new(cfg).await.expect("provider creation failed");

        // 未知角色 + AllowAll 默认策略 → 允许
        assert!(provider.check("guest", "any_table", PermissionAction::Select).await.unwrap());

        drop(provider);
        drop(dir);
    }

    #[tokio::test]
    async fn test_yaml_provider_get_policy() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let path = dir.path().join("policy.yaml");
        let yaml = r#"
editor:
  tables:
    - name: "articles"
      operations:
        - Select
        - Update
"#;
        std::fs::write(&path, yaml).expect("failed to write policy.yaml");

        let cfg = PermissionConfig {
            policy_path: Some(path.to_string_lossy().into_owned()),
            ..PermissionConfig::default()
        };
        let provider = new(cfg).await.expect("provider creation failed");

        // 已配置角色
        let policy = provider.get_policy("editor").await.unwrap();
        assert!(policy.is_some());
        let policy = policy.unwrap();
        assert_eq!(policy.tables.len(), 1);
        assert_eq!(policy.tables[0].name, "articles");

        // 未配置角色
        let missing = provider.get_policy("nonexistent").await.unwrap();
        assert!(missing.is_none());

        drop(provider);
        drop(dir);
    }

    #[tokio::test]
    async fn test_yaml_provider_refresh_reloads_file() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let path = dir.path().join("policy.yaml");
        // 初始：editor 只能 Select
        std::fs::write(&path, r#"
editor:
  tables:
    - name: "articles"
      operations:
        - Select
"#)
        .expect("failed to write policy.yaml");

        let cfg = PermissionConfig {
            policy_path: Some(path.to_string_lossy().into_owned()),
            ..PermissionConfig::default()
        };
        let provider = new(cfg).await.expect("provider creation failed");

        // 初始状态：editor 只能 Select
        assert!(provider.check("editor", "articles", PermissionAction::Select).await.unwrap());
        assert!(!provider.check("editor", "articles", PermissionAction::Update).await.unwrap());

        // 修改文件：editor 增加 Update 权限
        std::fs::write(&path, r#"
editor:
  tables:
    - name: "articles"
      operations:
        - Select
        - Update
"#)
        .expect("failed to write policy.yaml");

        // refresh 后应重新加载
        provider.refresh().await.expect("refresh failed");

        assert!(provider.check("editor", "articles", PermissionAction::Select).await.unwrap());
        assert!(provider.check("editor", "articles", PermissionAction::Update).await.unwrap());

        drop(provider);
        drop(dir);
    }

    #[tokio::test]
    async fn test_yaml_provider_health_check_and_shutdown() {
        let cfg = PermissionConfig::default();
        let provider = new(cfg).await.expect("provider creation failed");

        assert!(provider.health_check().await.is_ok());

        // shutdown 不应 panic
        provider.shutdown().await;

        // shutdown 后 admin 仍可访问
        assert!(provider.check("admin", "users", PermissionAction::Select).await.unwrap());
    }

    // =========================================================================
    // with_cache (cache feature)
    // =========================================================================

    #[cfg(feature = "cache")]
    #[tokio::test]
    async fn test_with_cache_injection() {
        let cache: Arc<oxcache::Cache<String, RolePolicy>> = Arc::new(oxcache::Cache::new());
        let cfg = PermissionConfig::default();
        let result = dbnexus::domain::permission::with_cache(cfg, cache).await;
        assert!(result.is_ok(), "with_cache should succeed with default config");
    }

    #[cfg(feature = "cache")]
    #[tokio::test]
    async fn test_with_cache_invalid_path() {
        let cache: Arc<oxcache::Cache<String, RolePolicy>> = Arc::new(oxcache::Cache::new());
        let cfg = PermissionConfig {
            policy_path: Some("/nonexistent/path/policy.yaml".into()),
            ..PermissionConfig::default()
        };
        let result = dbnexus::domain::permission::with_cache(cfg, cache).await;
        assert!(result.is_err());
    }

    // =========================================================================
    // 辅助：验证 PermissionProvider trait object 可用性
    // =========================================================================

    #[tokio::test]
    async fn test_permission_provider_as_dyn_trait() {
        let provider = new_in_memory();
        // 通过 trait object 验证 PermissionProvider 的所有 super-trait 都可用
        let checker: &dyn PermissionChecker = &provider;
        let manager: &dyn PolicyManager = &provider;
        let lifecycle: &dyn PermissionLifecycle = &provider;

        assert!(checker.check("admin", "users", PermissionAction::Select).await.is_ok());
        assert!(manager.get_policy("anyone").await.is_ok());
        assert!(lifecycle.health_check().await.is_ok());
        lifecycle.shutdown().await;
    }

    // =========================================================================
    // HashMap / Arc 使用（防止 unused import 警告）
    // =========================================================================

    #[test]
    fn test_hashmap_and_arc_are_used() {
        let _map: HashMap<String, RolePolicy> = HashMap::new();
        let _arc: Arc<RolePolicy> = Arc::new(RolePolicy::default());
    }
}
