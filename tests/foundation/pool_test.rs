// Foundation Pool Module Tests

#[cfg(feature = "pool")]
mod pool_tests {
    use dbnexus::foundation::pool::{PoolConfig, PoolConfigError};

    #[test]
    fn test_pool_config_validate_empty_url() {
        let config = PoolConfig::default();
        let result = config.validate();
        assert!(matches!(result, Err(PoolConfigError::MissingField(_))));
    }

    #[test]
    fn test_pool_config_validate_zero_max_connections() {
        let mut config = PoolConfig::default();
        config.url = "sqlite::memory:".to_string();
        config.max_connections = 0;
        let result = config.validate();
        assert!(matches!(result, Err(PoolConfigError::InvalidValue { .. })));
    }

    #[test]
    fn test_pool_config_validate_min_exceeds_max() {
        let mut config = PoolConfig::default();
        config.url = "sqlite::memory:".to_string();
        config.max_connections = 5;
        config.min_connections = 10;
        let result = config.validate();
        assert!(matches!(result, Err(PoolConfigError::InvalidValue { .. })));
    }

    #[test]
    fn test_pool_config_validate_success() {
        let mut config = PoolConfig::default();
        config.url = "sqlite::memory:".to_string();
        let result = config.validate();
        assert!(result.is_ok());
    }
}
