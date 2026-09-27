// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! inklog 结构化日志集成。
//!
//! 启用 `inklog` feature 后，[`init_inklog_logger`] 创建 inklog 的
//! `LoggerManager` 并将其安装为全局 `log` crate 后端。此后连接池、会话、
//! 健康检查与指标模块中的日志接线点（池获取超时、权限拒绝、熔断器状态
//! 转换、慢查询）经 `log` 宏发出的记录自动路由到 inklog 的结构化管道。
//!
//! # Sink 可达边界（结构性限制）
//!
//! 经 `log` 门面桥接的记录只到达 **console / file** 两个 sink；inklog 的
//! database sink 消费独立的原生 tracing 通道，log 门面记录**不会**入库。
//! 另外 [`InklogConfig::default()`] 仅启用 console sink，file/database
//! sink 需显式配置——只配置 database sink 的部署收不到任何 dbnexus 运维
//! 记录，且该缺失是静默的。生产环境建议启用 JSON 输出格式（text 模板
//! 原样渲染消息，依赖调用方消毒；dbnexus 接线点已对用户可控字段做控制
//! 字符消毒，JSON 模式额外提供 serde 转义兜底）。
//!
//! `inklog` feature 关闭时本模块不存在；接线点的 `log::` 宏调用仍无条件
//! 编译，在未安装任何 logger 时为 no-op——默认构建行为不变。
//!
//! # Manager 生命周期
//!
//! [`init_inklog_logger`] 返回的 `LoggerManager` 必须在应用存活期内保持
//! 存活。**manager 被 Drop 后 inklog 管道关闭（worker 退出、channel 断开），
//! 而全局 log 后端无法卸载**——此后所有接线点记录进入已断开通道被静默
//! 丢弃（inklog 仅内部计数，无错误上报），池超时/权限拒绝/熔断等运维
//! 关键事件失去观测信号。请将 manager 保存在应用主状态中而非临时变量。
//!
//! # Example
//!
//! ```rust,no_run
//! # #[cfg(feature = "inklog")]
//! # {
//! use dbnexus::integrations::inklog::{init_inklog_logger, InklogInit};
//!
//! # tokio::runtime::Runtime::new().expect("runtime").block_on(async {
//! let init = init_inklog_logger().await.expect("init inklog");
//! match init.is_installed() {
//!     true => log::info!("inklog installed as the global log backend"),
//!     false => log::info!("global log backend already installed; config unchanged"),
//! }
//! // 将 manager 存入应用主状态，保持存活：
//! let _manager = init.into_manager();
//! # });
//! # }
//! ```

use std::sync::atomic::{AtomicBool, Ordering};

/// 重导出 inklog 核心类型，供下游通过 `dbnexus::integrations::inklog::`
/// 直接访问，避免下游 crate 为类型引用单独声明 inklog 依赖。
pub use ::inklog::{InklogConfig, InklogError, LoggerManager};

/// [`init_inklog_logger`] / [`init_inklog_logger_with_config`] 的初始化结果。
///
/// 全局 `log` 后端每进程只能安装一次，返回值区分两种结局，让调用方可以
/// 感知「本次传入的配置是否生效」——对审计日志这类安全敏感配置，静默
/// 复用旧后端意味着级别/sink 变更未生效，必须显性化。
pub enum InklogInit {
    /// 本次调用完成了全局后端安装，传入的 [`InklogConfig`] 生效。
    Installed(LoggerManager),
    /// 本函数此前已成功初始化过（以本函数为观测点）：返回新创建的
    /// manager 以维持管道存活，但全局后端维持**首次**安装时的配置，
    /// 本次传入的配置被忽略。
    Reused(LoggerManager),
}

impl std::fmt::Debug for InklogInit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InklogInit::Installed(_) => f.write_str("InklogInit::Installed(LoggerManager)"),
            InklogInit::Reused(_) => f.write_str("InklogInit::Reused(LoggerManager)"),
        }
    }
}

impl InklogInit {
    /// 本次调用是否完成了全局后端安装（配置生效）。
    pub fn is_installed(&self) -> bool {
        matches!(self, InklogInit::Installed(_))
    }

    /// 取出 `LoggerManager`（无论哪种结局都须保持存活，见模块文档）。
    pub fn into_manager(self) -> LoggerManager {
        match self {
            InklogInit::Installed(m) | InklogInit::Reused(m) => m,
        }
    }

    /// 借用 `LoggerManager`。
    pub fn manager(&self) -> &LoggerManager {
        match self {
            InklogInit::Installed(m) | InklogInit::Reused(m) => m,
        }
    }
}

/// 进程级观测点：本模块是否已完成过一次成功初始化。
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// 单向标记：首次标记返回 true（配置生效），后续返回 false（复用既有后端）。
/// 抽为带状态参数的纯函数以便单元测试（进程级标志只能单向翻转）。
fn try_mark_initialized(flag: &AtomicBool) -> bool {
    flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

/// 将 inklog 初始化为全局结构化日志后端。
///
/// 使用默认配置创建 `LoggerManager`。等价于
/// [`init_inklog_logger_with_config`]`(InklogConfig::default())`。
///
/// 注意 [`InklogConfig::default()`] 仅启用 console sink（见模块文档的
/// Sink 可达边界）。
///
/// # 幂等性与配置生效范围
///
/// 全局 `log` logger 每进程只能安装一次。首次成功调用返回
/// [`InklogInit::Installed`]（配置生效）；后续调用返回
/// [`InklogInit::Reused`]——新配置**不生效**，全局后端维持首次安装时的
/// 级别/sink。需要变更配置时请重启进程或经 inklog 自身的运行时接口调整。
///
/// # Errors
///
/// 返回 `Err(InklogError)`：`LoggerManager` 构造失败（channel/sink 创建错误）。
pub async fn init_inklog_logger() -> Result<InklogInit, InklogError> {
    init_inklog_logger_with_config(InklogConfig::default()).await
}

/// 以自定义配置将 inklog 初始化为全局结构化日志后端。
///
/// 接受 [`InklogConfig`] 以控制日志级别、输出 sink（console / file）与
/// 按 target 的级别过滤。池/会话/健康/指标接线点的记录 target 为各自
/// 模块路径，可用 per-target 规则独立调控。
///
/// # 幂等性与配置生效范围
///
/// 仅首次成功调用的配置生效（返回 [`InklogInit::Installed`]）；后续调用
/// 返回 [`InklogInit::Reused`]，传入配置被忽略。若进程内已有其他组件
/// 先于本函数安装了全局 log 后端，inklog 的 install 会静默失败（降级为
/// debug 级内部日志，默认级别下不可见），此处仍报 `Installed`——该区分
/// 以本函数为观测点，不感知进程内的其他安装方。
///
/// # Errors
///
/// 返回 `Err(InklogError)`：`LoggerManager` 构造失败。
pub async fn init_inklog_logger_with_config(
    config: InklogConfig,
) -> Result<InklogInit, InklogError> {
    let manager = LoggerManager::with_config(config).await?;
    if try_mark_initialized(&INITIALIZED) {
        Ok(InklogInit::Installed(manager))
    } else {
        Ok(InklogInit::Reused(manager))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// init_inklog_logger() 构造 LoggerManager 成功。
    /// Installed/Reused 的区分由 try_mark_initialized 单测覆盖——
    /// 进程级标志是单向的，并行测试下无法断言谁是首次调用。
    #[tokio::test]
    async fn init_inklog_logger_returns_manager() {
        let result = init_inklog_logger().await;
        let init = result.expect("init_inklog_logger should return Ok");
        // manager 可用（管道存活），无论哪种结局
        let _manager = init.into_manager();
    }

    /// 标记纯逻辑：首次成功、后续报复用（配置不生效的显性信号）。
    #[test]
    fn try_mark_initialized_first_call_wins() {
        static FLAG: AtomicBool = AtomicBool::new(false);
        assert!(try_mark_initialized(&FLAG), "first mark must win");
        assert!(
            !try_mark_initialized(&FLAG),
            "second mark must report reuse (config ignored)"
        );
        assert!(
            try_mark_initialized(&AtomicBool::new(false)),
            "fresh flag marks as first"
        );
    }

    /// 初始化后全局 log 级别不再是 Off（证明 inklog 的 install 路径执行）。
    #[tokio::test]
    async fn init_sets_global_log_level() {
        let init = init_inklog_logger().await.expect("init should succeed");
        let _manager = init.into_manager();
        assert_ne!(
            log::max_level(),
            log::LevelFilter::Off,
            "global log level should be set by inklog, not remain Off"
        );
    }

    /// 初始化后接线点使用的 log:: 宏调用路由经 inklog 桥且不 panic。
    #[tokio::test]
    async fn log_calls_route_through_inklog_without_panic() {
        let init = init_inklog_logger().await.expect("init should succeed");
        let _manager = init.into_manager();
        // 覆盖接线点使用的全部级别；桥接错线时这里会 panic 或挂起。
        log::warn!("dbnexus inklog bridge test: warn level");
        log::info!("dbnexus inklog bridge test: info level");
    }
}
