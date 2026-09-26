// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! inklog 结构化日志集成。
//!
//! 启用 `inklog` feature 后，[`init_inklog_logger`] 创建 inklog 的
//! `LoggerManager` 并将其安装为全局 `log` crate 后端。此后连接池、会话、
//! 健康检查与指标模块中的日志接线点（池获取超时、权限拒绝、熔断器状态
//! 转换、慢查询）经 `log` 宏发出的记录自动路由到 inklog 的结构化管道
//! （console / file / database sinks）。
//!
//! `inklog` feature 关闭时本模块不存在；接线点的 `log::` 宏调用仍无条件
//! 编译，在未安装任何 logger 时为 no-op——默认构建行为不变。
//!
//! # Example
//!
//! ```rust,no_run
//! # #[cfg(feature = "inklog")]
//! # {
//! use dbnexus::integrations::inklog::init_inklog_logger;
//!
//! # tokio_test::block_on(async {
//! let _manager = init_inklog_logger().await.expect("init inklog");
//! log::info!("dbnexus records now route through inklog");
//! # });
//! # }
//! ```

/// 重导出 inklog 核心类型，供下游通过 `dbnexus::integrations::inklog::`
/// 直接访问，避免下游 crate 为类型引用单独声明 inklog 依赖。
pub use ::inklog::{InklogConfig, InklogError, LoggerManager};

/// 将 inklog 初始化为全局结构化日志后端。
///
/// 使用默认配置创建 `LoggerManager`。等价于
/// [`init_inklog_logger_with_config`]`(InklogConfig::default())`。
///
/// 返回的 `LoggerManager` 必须在应用存活期内保持存活，丢弃它会关闭
/// inklog 的日志管道。
///
/// # 幂等性
///
/// 全局 `log` logger 每进程只能安装一次。重复调用时首次完成安装，
/// 后续调用仍返回 `Ok`（install 失败被 inklog 降级为 warn）。
///
/// # Errors
///
/// 返回 `Err(InklogError)`：`LoggerManager` 构造失败（channel/sink 创建错误）。
pub async fn init_inklog_logger() -> Result<LoggerManager, InklogError> {
    init_inklog_logger_with_config(InklogConfig::default()).await
}

/// 以自定义配置将 inklog 初始化为全局结构化日志后端。
///
/// 接受 [`InklogConfig`] 以控制日志级别、输出 sink（console / file /
/// database）与按 target 的级别过滤。池/会话/健康/指标接线点的记录
/// target 为各自模块路径，可用 per-target 规则独立调控。
///
/// # Errors
///
/// 返回 `Err(InklogError)`：`LoggerManager` 构造失败。
pub async fn init_inklog_logger_with_config(
    config: InklogConfig,
) -> Result<LoggerManager, InklogError> {
    LoggerManager::with_config(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// init_inklog_logger() 构造 LoggerManager 成功。
    #[tokio::test]
    async fn init_inklog_logger_returns_manager() {
        let result = init_inklog_logger().await;
        assert!(
            result.is_ok(),
            "init_inklog_logger should return Ok, got: {:?}",
            result.err()
        );
    }

    /// 初始化后全局 log 级别不再是 Off（证明 inklog 的 install 路径执行）。
    #[tokio::test]
    async fn init_sets_global_log_level() {
        let _manager = init_inklog_logger().await.expect("init should succeed");
        assert_ne!(
            log::max_level(),
            log::LevelFilter::Off,
            "global log level should be set by inklog, not remain Off"
        );
    }

    /// 初始化后接线点使用的 log:: 宏调用路由经 inklog 桥且不 panic。
    #[tokio::test]
    async fn log_calls_route_through_inklog_without_panic() {
        let _manager = init_inklog_logger().await.expect("init should succeed");
        // 覆盖接线点使用的全部级别；桥接错线时这里会 panic 或挂起。
        log::warn!("dbnexus inklog bridge test: warn level");
        log::info!("dbnexus inklog bridge test: info level");
    }

    /// 重复初始化不 panic、仍返回 Ok（全局 logger 仅能安装一次，
    /// install 失败被 inklog 降级为 warn）。
    #[tokio::test]
    async fn init_inklog_logger_is_idempotent() {
        let _first = init_inklog_logger()
            .await
            .expect("first init should succeed");
        let second = init_inklog_logger().await;
        assert!(
            second.is_ok(),
            "second init should still return Ok (install failure is downgraded to warn), got: {:?}",
            second.err()
        );
    }
}
