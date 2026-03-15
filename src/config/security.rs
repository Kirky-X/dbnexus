// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 安全相关功能
//!
//! 包含 URL 验证、路径安全检查、脱敏函数等安全相关功能。

use std::path::Path;
use url::Url;

use super::types::ConfigError;

/// URL协议白名单（仅允许安全的数据库协议）
pub const ALLOWED_URL_SCHEMES: &[&str] = &["sqlite", "sqlite3", "postgres", "postgresql", "mysql", "mysql2"];

/// 敏感环境变量列表（加载时记录但不记录值）
pub const SENSITIVE_ENV_VARS: &[&str] = &[
    "DATABASE_URL",
    "DB_PASSWORD",
    "DB_PASSWORD_FILE",
    "DB_CERT",
    "DB_KEY",
    "DB_CA_CERT",
];

/// 环境变量最大长度限制
pub const MAX_ENV_VAR_LENGTH: usize = 4096;

/// 敏感参数键名列表
pub const SENSITIVE_QUERY_KEYS: [&str; 8] = ["password", "pass", "pwd", "key", "secret", "token", "apikey", "api_key"];

/// 对查询参数进行脱敏处理
pub fn sanitize_query_params(query: &str) -> String {
    query
        .split('&')
        .map(|param| {
            if let Some((key, _)) = param.split_once('=') {
                // 检查键名是否包含敏感关键词（不区分大小写）
                let key_lower = key.to_lowercase();
                if SENSITIVE_QUERY_KEYS.iter().any(|k| key_lower.contains(k)) {
                    return format!("{}=****", key);
                }
            }
            param.to_string()
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// 对 URL 进行脱敏处理，用于日志输出
///
/// 处理以下格式：
/// - 标准 URL: `postgres://user:pass@host/db` -> `postgres://****@host/db`
/// - SQLite 内存数据库: `sqlite::memory:` -> `sqlite::memory:`
/// - SQLite 带参数: `sqlite::memory:?password=secret` -> `sqlite::memory:?password=****`
/// - SQLite 文件: `sqlite:/path/to/db?key=value` -> `sqlite:/path/to/db?key=****`
pub fn sanitize_url_for_logging(url: &str) -> String {
    // 特殊处理 SQLite 内存数据库
    if url.starts_with("sqlite::memory:") || url.starts_with("sqlite3::memory:") {
        // 检查是否有查询参数
        if let Some(query_start) = url.find('?') {
            let base = &url[..query_start];
            let query = &url[query_start + 1..];
            let sanitized_query = sanitize_query_params(query);
            return format!("{}?{}", base, sanitized_query);
        }
        return url.to_string();
    }

    // 处理 SQLite 文件路径 URL
    if url.starts_with("sqlite:") || url.starts_with("sqlite3:") {
        // 检查是否有查询参数
        if let Some(query_start) = url.find('?') {
            let base = &url[..query_start];
            let query = &url[query_start + 1..];
            let sanitized_query = sanitize_query_params(query);
            return format!("{}?{}", base, sanitized_query);
        }
        return url.to_string();
    }

    // 处理标准的数据库 URL 格式：protocol://user:password@host:port/path
    // 注意：密码中可能包含 @ 符号，所以需要找到协议后最后一个 @
    if let Some(protocol_end) = url.find("://") {
        let protocol_part = &url[..protocol_end + 3];
        let after_protocol = &url[protocol_end + 3..];

        // 找到协议后最后一个 @ 符号（分隔用户信息和主机）
        if let Some(at_pos) = after_protocol.rfind('@') {
            let after_at = &after_protocol[at_pos + 1..];

            // 检查路径部分是否有查询参数需要脱敏
            let (host_path, sanitized_suffix) = if let Some(query_start) = after_at.find('?') {
                let host_path = &after_at[..query_start];
                let query = &after_at[query_start + 1..];
                let sanitized_query = sanitize_query_params(query);
                (host_path, format!("?{}", sanitized_query))
            } else {
                (after_at, String::new())
            };

            return format!("{}****@{}{}", protocol_part, host_path, sanitized_suffix);
        }
    }

    // 对于其他格式，检查是否有查询参数需要脱敏
    if let Some(query_start) = url.find('?') {
        let base = &url[..query_start];
        let query = &url[query_start + 1..];
        let sanitized_query = sanitize_query_params(query);
        return format!("{}?{}", base, sanitized_query);
    }

    url.to_string()
}

/// 验证数据库 URL 格式（增强版 - 包含协议白名单验证）
pub fn validate_url_format(url: &str) -> Result<(), ConfigError> {
    // 特殊处理 sqlite::memory: 和 sqlite3::memory: 格式（无 ://）
    if url.starts_with("sqlite::memory:") || url.starts_with("sqlite3::memory:") {
        return Ok(());
    }
    // 特殊处理 sqlite: 和 sqlite3: 格式（无 //）
    if url.starts_with("sqlite:") || url.starts_with("sqlite3:") {
        return Ok(());
    }

    // 使用 URL 解析器进行完整验证
    let parsed_url = Url::parse(url).map_err(|e| ConfigError::InvalidUrl(format!("Invalid URL format: {}", e)))?;

    let protocol = parsed_url.scheme();

    // 1. 检查协议格式（字母数字 + + . -）
    if !protocol
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
    {
        return Err(ConfigError::InvalidUrl(
            "Protocol contains invalid characters".to_string(),
        ));
    }

    // 2. 协议白名单验证（严格模式）
    let protocol_lower = protocol.to_lowercase();
    let is_valid_protocol = ALLOWED_URL_SCHEMES.contains(&protocol_lower.as_str())
        || (protocol_lower.starts_with("sqlite")
            && ["file", "mem", "memory"].contains(&protocol_lower.split(':').nth(1).unwrap_or("")));

    if !is_valid_protocol {
        // 记录无效协议拦截
        tracing::warn!(
            target: "security",
            "Blocked unsupported protocol '{}' in URL",
            protocol
        );
        return Err(ConfigError::UnsupportedProtocol);
    }

    // 3. 验证主机名格式（如果有）
    if let Some(host) = parsed_url.host() {
        let host_str = host.to_string();
        // 主机名不能包含空白字符或特殊符号（防止注入）
        if host_str
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '\'' | '"' | ';' | '|' | '&' | '$' | '`'))
        {
            return Err(ConfigError::InvalidUrl(
                "Hostname contains invalid characters".to_string(),
            ));
        }

        // 检查是否是IP地址（如果是，需要验证格式）
        if host_str.chars().all(|c| c.is_ascii_digit() || c == '.') {
            // 简单的IP格式验证
            let parts: Vec<&str> = host_str.split('.').collect();
            if parts.len() == 4 {
                for part in parts {
                    if part.parse::<u8>().is_err() {
                        return Err(ConfigError::InvalidUrl("Invalid IP address format".to_string()));
                    }
                }
            }
        }
    }

    // 4. 验证端口号范围（如果有）
    if let Some(port) = parsed_url.port() {
        if port == 0 {
            return Err(ConfigError::InvalidUrl(
                "Port number out of valid range (1-65535)".to_string(),
            ));
        }
    }

    // 5. 验证路径（如果有）- 防止SQL注入和路径遍历
    let path = parsed_url.path();
    if !path.is_empty() && (path.contains(';') || path.contains('\'') || path.contains('"')) {
        return Err(ConfigError::InvalidUrl(
            "URL path contains potentially dangerous characters".to_string(),
        ));
    }

    // 6. 验证用户名和密码（如果有）- 防止密码注入
    let user = parsed_url.username();
    if !user.is_empty() && (user.contains(':') || user.contains('@') || user.contains('/')) {
        return Err(ConfigError::InvalidUrl(
            "Username contains invalid characters".to_string(),
        ));
    }

    if let Some(password) = parsed_url.password() {
        if password.contains(':') || password.contains('@') || password.contains('/') {
            return Err(ConfigError::InvalidUrl(
                "Password contains invalid characters".to_string(),
            ));
        }
    }

    Ok(())
}

/// 安全地清理环境变量值
///
/// # Arguments
///
/// * `value` - 原始环境变量值
/// * `is_sensitive` - 是否是敏感值
///
/// # Returns
///
/// 清理后的值
pub fn sanitize_env_value(value: &str, is_sensitive: bool) -> String {
    // 检查长度
    if value.len() > MAX_ENV_VAR_LENGTH {
        tracing::warn!(
            target: "security",
            "Environment variable value exceeds maximum length, truncating"
        );
        return if is_sensitive {
            "[REDACTED - TOO LONG]".to_string()
        } else {
            value.chars().take(MAX_ENV_VAR_LENGTH).collect()
        };
    }

    // 移除潜在的恶意字符
    let sanitized: String = value
        .chars()
        .filter(|c| {
            // 移除null字节、控制字符
            !c.is_control() && *c != '\0'
        })
        .collect();

    if is_sensitive {
        "[REDACTED]".to_string()
    } else {
        sanitized
    }
}

/// 检查环境变量是否是敏感的
pub fn is_sensitive_env_var(name: &str) -> bool {
    SENSITIVE_ENV_VARS
        .iter()
        .any(|sensitive| name.eq_ignore_ascii_case(sensitive))
}

/// 检查配置文件路径是否安全（增强版 - 包含多层防护）
///
/// 防止路径遍历攻击：
/// - 检查路径是否包含父目录引用 (..)
/// - 检查路径是否包含符号链接
/// - 检查路径是否在预期目录内
/// - 检查 Windows 风格路径遍历
/// - 检查 null 字节注入
/// - 检查环境变量扩展攻击
/// - 检查特殊字符注入
pub fn is_safe_config_path(path: &Path) -> Result<bool, ConfigError> {
    // 1. 检查 null 字节注入
    let path_str = path.to_string_lossy();
    if path_str.contains('\0') {
        tracing::warn!("Rejected config path with null byte: {:?}", path);
        return Ok(false);
    }

    // 2. 检查环境变量扩展（防止 $VAR 或 ${VAR} 形式的攻击）
    if path_str.contains("$(") || path_str.contains("${") || path_str.contains("`") {
        tracing::warn!("Rejected config path with environment variable expansion: {:?}", path);
        return Ok(false);
    }

    // 3. 检查路径是否包含 ..（父目录遍历）
    if path_str.contains("..") {
        tracing::warn!("Rejected config path with parent directory traversal: {:?}", path);
        return Ok(false);
    }

    // 4. 检查 Windows 风格路径遍历
    if path_str.contains(".\\") || path_str.starts_with(".\\") {
        tracing::warn!("Rejected config path with Windows-style traversal: {:?}", path);
        return Ok(false);
    }

    // 5. 检查特殊字符（可能用于路径注入）
    let dangerous_chars = [';', '|', '&', '>', '<', '*', '?', '~'];
    for c in dangerous_chars {
        if path_str.contains(c) {
            tracing::warn!("Rejected config path with dangerous character '{}': {:?}", c, path);
            return Ok(false);
        }
    }

    // 6. 检查是否是绝对路径
    if path.is_absolute() {
        // 检查是否在允许的目录范围内
        if let Ok(canonical) = path.canonicalize() {
            // 检查是否在用户目录或当前目录
            let home_dir = home::home_dir();
            let current_dir = std::env::current_dir().ok();

            let is_in_allowed_location = home_dir
                .as_ref()
                .is_some_and(|home| canonical.starts_with(home) || canonical.starts_with(home.join(".config")))
                || current_dir.as_ref().is_some_and(|current| {
                    canonical.starts_with(current) || canonical.starts_with(current.join("config"))
                });

            if !is_in_allowed_location {
                // 检查是否在系统关键目录
                let forbidden_prefixes = [
                    "/etc", "/usr", "/var", "/root", "/boot", "/srv", "/opt", "/bin", "/sbin", "/lib", "/lib64",
                    "/proc", "/sys", "/dev",
                ];
                for prefix in &forbidden_prefixes {
                    if canonical.starts_with(prefix) {
                        tracing::warn!("Rejected config path in system directory: {:?}", path);
                        return Ok(false);
                    }
                }
            }
        }
    }

    // 7. 规范化路径并检查
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to canonicalize config path {:?}: {}", path, e);
            return Ok(false);
        }
    };

    // 8. 检查符号链接（指向不安全位置的符号链接）
    if path.is_symlink() {
        tracing::warn!("Rejected symlink config path: {:?}", path);
        return Ok(false);
    }

    // 9. 检查规范化后的路径是否仍然包含 ..
    if canonical.to_string_lossy().contains("..") {
        tracing::warn!(
            "Rejected config path with hidden traversal after canonicalization: {:?}",
            path
        );
        return Ok(false);
    }

    // 10. 检查路径是否指向目录（配置文件应该是文件）
    if canonical.is_dir() {
        tracing::warn!("Rejected config path pointing to directory: {:?}", path);
        return Ok(false);
    }

    // 11. 检查文件扩展名是否安全
    if let Some(ext) = path.extension() {
        if ext == "sh" || ext == "bash" || ext == "py" || ext == "js" {
            tracing::warn!("Rejected config path with dangerous extension: {:?}", path);
            return Ok(false);
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-U-012: URL 脱敏测试 - 标准 URL 格式
    #[test]
    fn test_sanitize_url_standard() {
        // 标准 PostgreSQL URL
        assert_eq!(
            sanitize_url_for_logging("postgres://user:password@localhost:5432/mydb"),
            "postgres://****@localhost:5432/mydb"
        );

        // MySQL URL
        assert_eq!(
            sanitize_url_for_logging("mysql://admin:secret123@db.example.com:3306/production"),
            "mysql://****@db.example.com:3306/production"
        );

        // 没有密码的 URL
        assert_eq!(
            sanitize_url_for_logging("postgres://user@localhost/mydb"),
            "postgres://****@localhost/mydb"
        );
    }

    /// TEST-U-013: URL 脱敏测试 - SQLite 内存数据库
    #[test]
    fn test_sanitize_url_sqlite_memory() {
        // SQLite 内存数据库 - 无参数
        assert_eq!(
            sanitize_url_for_logging("sqlite::memory:"),
            "sqlite::memory:"
        );

        // SQLite3 内存数据库 - 无参数
        assert_eq!(
            sanitize_url_for_logging("sqlite3::memory:"),
            "sqlite3::memory:"
        );

        // SQLite 内存数据库 - 带敏感参数
        assert_eq!(
            sanitize_url_for_logging("sqlite::memory:?password=secret"),
            "sqlite::memory:?password=****"
        );

        // SQLite 内存数据库 - 带多个参数
        assert_eq!(
            sanitize_url_for_logging("sqlite::memory:?password=secret&mode=memory&cache=shared"),
            "sqlite::memory:?password=****&mode=memory&cache=shared"
        );
    }

    /// TEST-U-014: URL 脱敏测试 - SQLite 文件路径
    #[test]
    fn test_sanitize_url_sqlite_file() {
        // SQLite 文件路径 - 无参数
        assert_eq!(
            sanitize_url_for_logging("sqlite:/path/to/database.db"),
            "sqlite:/path/to/database.db"
        );

        // SQLite 文件路径 - 带敏感参数
        assert_eq!(
            sanitize_url_for_logging("sqlite:/path/to/db?key=mysecretkey"),
            "sqlite:/path/to/db?key=****"
        );

        // SQLite 文件路径 - 带多个参数
        assert_eq!(
            sanitize_url_for_logging("sqlite:/data/app.db?secret=abc123&readonly=true"),
            "sqlite:/data/app.db?secret=****&readonly=true"
        );

        // SQLite3 文件路径
        assert_eq!(
            sanitize_url_for_logging("sqlite3:/var/data/test.db?token=xyz789"),
            "sqlite3:/var/data/test.db?token=****"
        );
    }

    /// TEST-U-015: URL 脱敏测试 - 敏感参数关键词
    #[test]
    fn test_sanitize_url_sensitive_keywords() {
        // password
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?password=mypassword"),
            "postgres://****@host/db?password=****"
        );

        // pass
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?pass=mypass"),
            "postgres://****@host/db?pass=****"
        );

        // pwd
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?pwd=mypwd"),
            "postgres://****@host/db?pwd=****"
        );

        // key
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?key=mykey"),
            "postgres://****@host/db?key=****"
        );

        // secret
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?secret=mysecret"),
            "postgres://****@host/db?secret=****"
        );

        // token
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?token=mytoken"),
            "postgres://****@host/db?token=****"
        );

        // apikey
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?apikey=myapikey"),
            "postgres://****@host/db?apikey=****"
        );

        // api_key
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?api_key=myapikey"),
            "postgres://****@host/db?api_key=****"
        );
    }

    /// TEST-U-016: URL 脱敏测试 - 大小写不敏感
    #[test]
    fn test_sanitize_url_case_insensitive() {
        // 大写
        assert_eq!(
            sanitize_url_for_logging("sqlite::memory:?PASSWORD=secret"),
            "sqlite::memory:?PASSWORD=****"
        );

        // 混合大小写
        assert_eq!(
            sanitize_url_for_logging("sqlite::memory:?PaSsWoRd=secret"),
            "sqlite::memory:?PaSsWoRd=****"
        );

        // API_KEY 大写
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?API_KEY=secret"),
            "postgres://****@host/db?API_KEY=****"
        );
    }

    /// TEST-U-017: URL 脱敏测试 - 非敏感参数保留
    #[test]
    fn test_sanitize_url_preserve_non_sensitive() {
        // 非敏感参数应该保留原值
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?timeout=30&ssl=true"),
            "postgres://****@host/db?timeout=30&ssl=true"
        );

        // 混合敏感和非敏感参数
        assert_eq!(
            sanitize_url_for_logging("postgres://user:pass@host/db?timeout=30&password=secret&ssl=true"),
            "postgres://****@host/db?timeout=30&password=****&ssl=true"
        );

        // SQLite 文件路径 - 非敏感参数
        assert_eq!(
            sanitize_url_for_logging("sqlite:/path/to/db?mode=ro&cache=private"),
            "sqlite:/path/to/db?mode=ro&cache=private"
        );
    }

    /// TEST-U-018: URL 脱敏测试 - 边界情况
    #[test]
    fn test_sanitize_url_edge_cases() {
        // 空字符串
        assert_eq!(sanitize_url_for_logging(""), "");

        // 只有协议
        assert_eq!(sanitize_url_for_logging("postgres://"), "postgres://");

        // 空查询参数
        assert_eq!(sanitize_url_for_logging("sqlite::memory:?"), "sqlite::memory:?");

        // 空参数值
        assert_eq!(
            sanitize_url_for_logging("sqlite::memory:?password="),
            "sqlite::memory:?password=****"
        );

        // 没有 @ 符号的标准 URL
        assert_eq!(
            sanitize_url_for_logging("postgres://localhost:5432/mydb"),
            "postgres://localhost:5432/mydb"
        );

        // 包含特殊字符的密码
        assert_eq!(
            sanitize_url_for_logging("postgres://user:p@ss:w0rd@host/db"),
            "postgres://****@host/db"
        );
    }

    /// TEST-U-019: URL 脱敏测试 - 复杂场景
    #[test]
    fn test_sanitize_url_complex() {
        // 标准 URL + 查询参数中的敏感信息
        assert_eq!(
            sanitize_url_for_logging("postgres://admin:secret@db.example.com:5432/production?sslmode=require&password=backup_key"),
            "postgres://****@db.example.com:5432/production?sslmode=require&password=****"
        );

        // 多个敏感参数
        assert_eq!(
            sanitize_url_for_logging("sqlite::memory:?password=pass1&token=token1&key=key1&normal=value"),
            "sqlite::memory:?password=****&token=****&key=****&normal=value"
        );
    }
}
