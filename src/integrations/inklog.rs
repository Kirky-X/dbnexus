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
//! 承载全局 `log` 门面记录的是**首次安装时的 `LoggerManager`**——每次
//! `LoggerManager::with_config` 构建的是独立管道，只有经全局 install 的
//! 那条管道接得住门面记录。本模块以 `Weak` 句柄跟踪该 manager：
//!
//! - 首个 manager **存活期间**，重复调用交还它的 `Arc` 句柄
//!   （[`InklogInit::Reused`]）——同一承载管道，不是孤儿新管道；
//! - 句柄被全部丢弃（inklog shutdown 排空、worker 退出）后再次调用，
//!   走完整重建并如实报告 [`InklogInit::Installed`]——但 inklog 的全局
//!   install 无法撤销，重建管道不再承接全局门面记录，全局后端仍指向
//!   已关闭的旧管道。**因此请把首个 manager 存入应用主状态、存活整个
//!   进程**；中途丢弃会导致后续运维记录静默丢失（inklog 仅内部计数，
//!   无错误上报）。
//!
//! 该语义同时避免一个上游缺陷：对既有活管道二次调用
//! `LoggerManager::with_config` 会死锁，且新建管道收不到门面记录——
//! 本模块的复用快路径让二次初始化根本不会再触碰 `with_config`。
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

use std::sync::{Arc, Mutex, Weak};

/// 重导出 inklog 核心类型，供下游通过 `dbnexus::integrations::inklog::`
/// 直接访问，避免下游 crate 为类型引用单独声明 inklog 依赖。
pub use ::inklog::{InklogConfig, InklogError, LoggerManager};

/// [`init_inklog_logger`] / [`init_inklog_logger_with_config`] 的初始化结果。
///
/// 全局 `log` 后端在首个 manager 存活期间只能安装一次，返回值区分两种
/// 结局，让调用方可以感知「本次传入的配置是否生效」——对审计日志这类
/// 安全敏感配置，静默复用旧后端意味着级别/sink 变更未生效，必须显性化。
pub enum InklogInit {
    /// 本次调用完成了全局后端安装（首次初始化，或先前管道已全部关闭后的
    /// 重建），传入的 [`InklogConfig`] 生效。
    Installed(Arc<LoggerManager>),
    /// 承载全局门面记录的 manager 仍然存活：返回它的 `Arc` 克隆——同一
    /// 承载管道，绝非收不到记录的孤儿新管道；本次传入的配置被忽略。
    Reused(Arc<LoggerManager>),
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

    /// 取出承载管道的句柄。
    pub fn into_manager(self) -> Arc<LoggerManager> {
        match self {
            InklogInit::Installed(m) | InklogInit::Reused(m) => m,
        }
    }

    /// 克隆承载管道的 `Arc` 句柄。
    pub fn manager_handle(&self) -> Arc<LoggerManager> {
        match self {
            InklogInit::Installed(m) | InklogInit::Reused(m) => Arc::clone(m),
        }
    }

    /// 借用承载管道的 `LoggerManager`。
    pub fn manager(&self) -> &LoggerManager {
        match self {
            InklogInit::Installed(m) | InklogInit::Reused(m) => m,
        }
    }
}

/// 承载全局 log 门面记录的 manager 的弱引用。Weak 而非 Arc：inklog 以
/// manager Drop 驱动 shutdown 排空，本模块若进程级持有强引用会让管道
/// 永活、破坏该契约（在 tokio 测试等 runtime 作用域环境直接死锁）。
static FIRST_MANAGER: Mutex<Option<Weak<LoggerManager>>> = Mutex::new(None);

/// 快路径复用：首次 manager 仍存活时交还其句柄；已全部丢弃（管道关闭）
/// 时返回 None，调用方走完整重建。
fn live_first_manager() -> Option<Arc<LoggerManager>> {
    let guard = FIRST_MANAGER.lock().expect("FIRST_MANAGER lock poisoned");
    let weak = guard.as_ref()?;
    weak.upgrade()
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
/// 首个 manager 存活期间，全局 `log` logger 每进程只能安装一次：后续
/// 调用返回 [`InklogInit::Reused`]——交还首次 manager 的句柄，新配置
/// **不生效**（级别/sink 维持首次安装时的值）。需要变更配置时请重启
/// 进程或经 inklog 自身的运行时接口调整。若首次 manager 的全部句柄都
/// 被丢弃（管道关闭），再次调用会重新初始化并返回
/// [`InklogInit::Installed`]——但 inklog 的全局 install 无法撤销，新
/// 管道不再承接全局门面记录（见模块文档 Sink 可达边界）。
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
/// 首个 manager 存活期间：后续调用交还首次 manager 的句柄
/// （[`InklogInit::Reused`]），传入配置被忽略，且**不会**再构建新管道
/// ——上游 inklog 在既有活管道时二次初始化会死锁，新建的管道也收不到
/// 全局门面记录。若进程内已有其他组件先于本函数安装了全局 log 后端，
/// inklog 的 install 会静默失败（降级为 debug 级内部日志，默认级别下
/// 不可见），此处仍报 `Installed`——该区分以本函数为观测点，不感知
/// 进程内的其他安装方。
///
/// # Errors
///
/// 返回 `Err(InklogError)`：`LoggerManager` 构造失败。
pub async fn init_inklog_logger_with_config(
    config: InklogConfig,
) -> Result<InklogInit, InklogError> {
    // 复用快路径：首次 manager 存活则直接交还句柄，不再二次 with_config
    // （上游在既有活管道时二次初始化死锁，且新管道是收不到门面记录的孤儿）
    if let Some(first) = live_first_manager() {
        return Ok(InklogInit::Reused(first));
    }

    let manager = Arc::new(LoggerManager::with_config(config).await?);
    let mut guard = FIRST_MANAGER.lock().expect("FIRST_MANAGER lock poisoned");
    // 双检：await 构建期间另一调用可能已装入存活 manager——交还它的
    // 句柄并丢弃本管道（避免孤儿管道）；仍无存活者则本管道接管。
    if let Some(existing) = guard.as_ref().and_then(|w| w.upgrade()) {
        return Ok(InklogInit::Reused(existing));
    }
    *guard = Some(Arc::downgrade(&manager));
    Ok(InklogInit::Installed(manager))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 进程级 init 状态（FIRST_MANAGER/全局 log 后端）在并行测试间共享，
    /// init 语义用例必须互斥执行。
    static INIT_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// init_inklog_logger() 构造 LoggerManager 成功。Installed/Reused
    /// 的语义由下方两个专用测试覆盖（进程级状态无法在并行测试中固定
    /// 谁是首次调用）。
    #[tokio::test]
    async fn init_inklog_logger_returns_manager() {
        let _guard = INIT_TEST_LOCK.lock().await;
        let result = init_inklog_logger().await;
        let init = result.expect("init_inklog_logger should return Ok");
        // manager 可用（管道存活），无论哪种结局
        let _manager = init.into_manager();
    }

    /// 初始化后全局 log 级别不再是 Off（证明 inklog 的 install 路径执行）。
    #[tokio::test]
    async fn init_sets_global_log_level() {
        let _guard = INIT_TEST_LOCK.lock().await;
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
        let _guard = INIT_TEST_LOCK.lock().await;
        let init = init_inklog_logger().await.expect("init should succeed");
        let _manager = init.into_manager();
        // 覆盖接线点使用的全部级别；桥接错线时这里会 panic 或挂起。
        log::warn!("dbnexus inklog bridge test: warn level");
        log::info!("dbnexus inklog bridge test: info level");
    }

    /// 首个 manager 存活期间：后续调用交还同一承载管道的句柄（Reused），
    /// 绝不产生收不到门面记录的孤儿新管道。
    #[tokio::test]
    async fn reused_returns_the_same_carrying_manager() {
        let _guard = INIT_TEST_LOCK.lock().await;
        let first = init_inklog_logger().await.expect("first init");
        let second = init_inklog_logger().await.expect("second init");
        assert!(
            !second.is_installed(),
            "in-order second init must report Reused (reuse fast path)"
        );
        assert!(
            std::ptr::eq(first.manager(), second.manager()),
            "both init results must hand out the same carrying manager"
        );
    }

    /// 首个 manager 的全部句柄被丢弃（管道关闭）后，再次调用走完整
    /// 重建并如实报告 Installed——而非交还已死管道的失效句柄。
    #[tokio::test]
    async fn reinit_after_drop_reports_installed() {
        let _guard = INIT_TEST_LOCK.lock().await;
        let first = init_inklog_logger().await.expect("first init");
        let first_ptr = Arc::as_ptr(&first.manager_handle());
        // 丢弃全部句柄：FIRST_MANAGER 只存 Weak，引用计数归零触发 inklog
        // shutdown（worker 退出、管道关闭）
        drop(first);
        if let Some(w) = FIRST_MANAGER
            .lock()
            .expect("FIRST_MANAGER lock poisoned")
            .as_ref()
        {
            assert!(w.upgrade().is_none(), "dropped manager must be released");
        }

        let second = init_inklog_logger().await.expect("second init");
        assert!(
            second.is_installed(),
            "after full drop the re-init must report Installed (fresh pipe)"
        );
        assert_ne!(
            Arc::as_ptr(&second.manager_handle()),
            first_ptr,
            "re-init must build a fresh pipe, not resurrect the dropped one"
        );
    }
}
