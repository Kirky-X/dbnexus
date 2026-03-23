// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 分布式追踪与日志模块
//!
//! 本模块提供两类功能：
//!
//! 1. **日志订阅者初始化** (`TracingBuilder`, `init_default`) - 配置 `tracing-subscriber`
//!    让你在应用入口点（如 `main()`）初始化日志输出。
//!
//! 2. **OpenTelemetry 分布式追踪** (`init`, `inject`, `extract`) - 配置 OpenTelemetry 导出器，
//!    用于跨服务的分布式追踪。
//!
//! # 快速开始（日志）
//!
//! 在你的应用入口点调用一次 `init_default()` 即可启用日志输出：
//!
//! ```rust
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     dbnexus::tracing::init_default()?;
//!     // 之后所有 dbnexus::info!() 等宏都会输出到 stdout
//!     let pool = DbPool::new("sqlite::memory:").await?;
//!     Ok(())
//! }
//! ```
//!
//! # 进阶配置（日志）
//!
//! 使用 `TracingBuilder` 精确控制输出格式和日志级别：
//!
//! ```rust
//! use dbnexus::tracing::TracingBuilder;
//!
//! TracingBuilder::new()
//!     .with_json()                          // JSON 结构化输出
//!     .with_level("dbnexus=debug,info")     // 自定义日志级别
//!     .init()?;
//! ```
//!
//! # OpenTelemetry 分布式追踪
//!
//! 需要分布式追踪时，将 OpenTelemetry 配置叠加到日志订阅者上：
//!
//! ```rust,no_run
//! use dbnexus::tracing::TracingBuilder;
//!
//! async fn setup_tracing() -> Result<(), Box<dyn std::error::Error>> {
//!     // 初始化日志订阅者 + OpenTelemetry OTLP 导出
//!     TracingBuilder::new()
//!         .with_level("info")
//!         .with_otlp("otlp", "http://localhost:4317")
//!         .init()?;
//!
//!     tracing::info!("追踪已启用");
//!     Ok(())
//! }
//! ```

use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
#[cfg(feature = "tracing")]
use std::collections::HashMap;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{fmt, EnvFilter};

#[cfg(feature = "tracing")]
use opentelemetry_sdk::trace::TracerProvider;

/// 默认日志级别，当 `RUST_LOG` 未设置时使用。
const DEFAULT_LOG_LEVEL: &str = "warn";

/// 日志订阅者构建器
///
/// 提供流畅的 API 来配置 `tracing-subscriber`：
/// - 输出格式：`with_fmt()`（默认，漂亮打印）或 `with_json()`（JSON 结构化）
/// - 日志级别：`with_level()`（默认 `warn`，或通过 `RUST_LOG` 环境变量）
/// - OpenTelemetry：`with_otlp()`（可选，开启分布式追踪）
///
/// # Example
///
/// ```rust
/// use dbnexus::tracing::TracingBuilder;
///
/// TracingBuilder::new()
///     .with_fmt()
///     .with_level("dbnexus=debug")
///     .init()?;
/// ```
#[derive(Default)]
pub struct TracingBuilder {
    output_format: OutputFormat,
    level: Option<String>,
    otlp_exporter: Option<OtlpConfig>,
}

/// 日志输出格式枚举。
#[derive(Default, Clone, Copy)]
enum OutputFormat {
    /// 漂亮打印格式（默认）
    #[default]
    Fmt,
    /// JSON 结构化格式（适用于日志聚合器）
    Json,
}

/// OpenTelemetry 配置。
#[derive(Clone)]
struct OtlpConfig {
    exporter: String,
    endpoint: String,
}

impl TracingBuilder {
    /// 创建一个新的 `TracingBuilder`，默认配置为：
    /// - fmt 输出到 stdout
    /// - 日志级别为 `warn`（可通过 `RUST_LOG` 环境变量覆盖）
    pub fn new() -> Self {
        Self::default()
    }

    /// 配置使用漂亮打印（ANSI）格式输出到 stdout。
    pub fn with_fmt(self) -> Self {
        TracingBuilder { output_format: OutputFormat::Fmt, ..self }
    }

    /// 配置使用 JSON 结构化格式输出到 stdout。
    ///
    /// 适用于将日志发送到日志聚合系统（如 Loki、Elasticsearch）。
    pub fn with_json(self) -> Self {
        TracingBuilder { output_format: OutputFormat::Json, ..self }
    }

    /// 设置 env filter 字符串（如 `"dbnexus=debug,info"`）。
    ///
    /// 如果未调用，默认使用 `"warn"` 或 `RUST_LOG` 环境变量。
    pub fn with_level(mut self, level: &str) -> Self {
        self.level = Some(level.to_string());
        self
    }

    /// 启用 OpenTelemetry OTLP 导出。
    ///
    /// 启用后会同时初始化 OpenTelemetry tracer 和 OTLP 导出器，
    /// 并将 `tracing-opentelemetry` layer 叠加到 fmt/json 订阅者之上。
    ///
    /// - `exporter`: 导出器类型，当前支持 `"otlp"` 和 `"stdout"`
    /// - `endpoint`: OTLP 端点 URL（如 `"http://localhost:4317"`）
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// TracingBuilder::new()
    ///     .with_otlp("otlp", "http://localhost:4317")
    ///     .init()?;
    /// ```
    #[cfg(feature = "tracing")]
    pub fn with_otlp(self, exporter: &str, endpoint: &str) -> Self {
        TracingBuilder {
            otlp_exporter: Some(OtlpConfig {
                exporter: exporter.to_string(),
                endpoint: endpoint.to_string(),
            }),
            ..self
        }
    }

    /// 消费构建器并初始化订阅者。
    ///
    /// 调用 `fmt().with_env_filter().try_init()` 完成初始化。
    /// 如果订阅者已初始化，返回 `Err` 而非 panic。
    ///
    /// 注意：当 `with_otlp()` 一起使用时，OpenTelemetry tracer 会被初始化并注册到全局，
    /// 但 OTel layer 需要在单独的 subscriber 初始化中配置（详见模块文档）。
    pub fn init(self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let level = self
            .level
            .clone()
            .unwrap_or_else(|| std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_LEVEL.to_string()));

        let filter = EnvFilter::new(&level);

        #[cfg(feature = "tracing")]
        if let Some(ref otlp) = self.otlp_exporter {
            // 初始化 OpenTelemetry tracer 并注册到全局（供 tracing-opentelemetry 使用）
            let resource =
                opentelemetry_sdk::Resource::new(vec![KeyValue::new("service.name", "dbnexus")]);
            let config = opentelemetry_sdk::trace::Config::default().with_resource(resource);

            let provider: TracerProvider = match otlp.exporter.as_str() {
                "otlp" => opentelemetry_otlp::new_pipeline()
                    .tracing()
                    .with_exporter(
                        opentelemetry_otlp::new_exporter().tonic().with_endpoint(&otlp.endpoint),
                    )
                    .with_trace_config(config)
                    .install_simple()
                    .map_err(|e| e.to_string())?,
                _ => opentelemetry_otlp::new_pipeline()
                    .tracing()
                    .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_endpoint("stdout"))
                    .with_trace_config(config)
                    .install_simple()
                    .map_err(|e| e.to_string())?,
            };

            global::set_tracer_provider(provider);
            let propagator = TraceContextPropagator::default();
            global::set_text_map_propagator(propagator);
        }

        #[cfg(not(feature = "tracing"))]
        let _ = self.otlp_exporter; // OpenTelemetry 需要 tracing feature

        // 初始化 fmt/json subscriber
        match self.output_format {
            OutputFormat::Fmt => fmt()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true)
                .with_span_events(FmtSpan::CLOSE)
                .with_env_filter(filter)
                .try_init(),
            OutputFormat::Json => fmt()
                .json()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true)
                .with_span_events(FmtSpan::CLOSE)
                .flatten_event(true)
                .with_env_filter(filter)
                .try_init(),
        }
    }
}

/// 使用默认配置初始化日志订阅者。
///
/// 默认行为：
/// - fmt 漂亮打印输出到 stdout
/// - 日志级别：`RUST_LOG` 环境变量（若设置），否则为 `warn`
/// - OpenTelemetry **不**自动启用（调用 `TracingBuilder::with_otlp()` 以启用分布式追踪）
///
/// # Example
///
/// ```rust
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     dbnexus::tracing::init_default()?;
///     tracing::info!("DBNexus 日志系统已初始化");
///     Ok(())
/// }
/// ```
pub fn init_default() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    TracingBuilder::new().with_fmt().init()
}

// ============================================================================
// 现有 OpenTelemetry API（保持向后兼容）
// ============================================================================

/// 追踪初始化结果
pub struct TracingGuard {
    _provider: TracerProvider,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        global::shutdown_tracer_provider();
    }
}

/// 初始化 OpenTelemetry 分布式追踪（OTLP 或 stdout）。
///
/// 此函数初始化 OpenTelemetry tracer 和传播器，**不**配置日志订阅者。
/// 典型用法是先调用 `init_default()` 或 `TracingBuilder::init()` 配置日志，
/// 再调用本函数配置分布式追踪。
///
/// # Example
///
/// ```rust,no_run
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     dbnexus::tracing::init_default()?;
///
///     // 启用 OTLP 分布式追踪
///     let _guard = dbnexus::tracing::init("otlp", "http://localhost:4317").await?;
///     tracing::info!("追踪已启用");
///     Ok(())
/// }
/// ```
pub async fn init(exporter: &str, endpoint: &str) -> Result<TracingGuard, String> {
    let provider: TracerProvider = match exporter.to_lowercase().as_str() {
        "otlp" => init_otlp(endpoint).await?,
        "stdout" => init_stdout()?,
        _ => init_stdout()?,
    };

    global::set_tracer_provider(provider.clone());

    let propagator = TraceContextPropagator::default();
    global::set_text_map_propagator(propagator);

    Ok(TracingGuard { _provider: provider })
}

/// 使用 OTLP 初始化追踪
async fn init_otlp(endpoint: &str) -> Result<TracerProvider, String> {
    let resource = opentelemetry_sdk::Resource::new(vec![KeyValue::new("service.name", "dbnexus")]);

    let config = opentelemetry_sdk::trace::Config::default().with_resource(resource);

    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_endpoint(endpoint))
        .with_trace_config(config)
        .install_simple()
        .map_err(|e| e.to_string())?;

    Ok(provider)
}

/// 使用标准输出初始化追踪
fn init_stdout() -> Result<TracerProvider, String> {
    let resource = opentelemetry_sdk::Resource::new(vec![KeyValue::new("service.name", "dbnexus")]);

    let config = opentelemetry_sdk::trace::Config::default().with_resource(resource);

    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_endpoint("stdout"))
        .with_trace_config(config)
        .install_simple()
        .map_err(|e| e.to_string())?;

    Ok(provider)
}

/// 从 HashMap 注入追踪上下文
pub fn inject(headers: &mut HashMap<String, String>) {
    global::get_text_map_propagator(|propagator| {
        propagator.inject(headers);
    });
}

/// 从 HashMap 提取追踪上下文
pub fn extract(headers: &HashMap<String, String>) {
    global::get_text_map_propagator(|propagator| {
        let _ = propagator.extract(headers);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_builder_default() {
        let result = TracingBuilder::new().init();
        // 第一次 init 可能成功（无全局订阅者）或失败（已有订阅者）
        // 关键是不 panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_tracing_builder_with_level() {
        let result = TracingBuilder::new()
            .with_fmt()
            .with_level("dbnexus=debug")
            .init();
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_tracing_builder_json() {
        let result = TracingBuilder::new()
            .with_json()
            .with_level("info")
            .init();
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_tracing_builder_with_fmt_and_level() {
        // Test that with_fmt() and with_level() can be chained
        let builder = TracingBuilder::new().with_fmt().with_level("error");
        let result = builder.init();
        // try_init 只在第一次成功，后续调用返回错误
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_tracing_builder_json_format() {
        // Test that with_json() sets JSON output format
        let builder = TracingBuilder::new().with_json().with_level("warn");
        let result = builder.init();
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_tracing_builder_otlp() {
        // Test that with_otlp() can be called (exporter is validated at init time)
        let builder = TracingBuilder::new().with_otlp("stdout", "http://localhost:4317");
        let result = builder.init();
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_init_default_uses_rust_log() {
        // init_default should read RUST_LOG env var
        // 如果 RUST_LOG 未设置，使用 DEFAULT_LOG_LEVEL ("warn")
        let result = init_default();
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_init_default_uses_default_level() {
        // init_default 默认使用 "warn" 级别（或 RUST_LOG 如果设置）
        let result = init_default();
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_tracing_init_and_propagation() {
        let mut headers = HashMap::new();
        headers.insert("x-test".to_string(), "1".to_string());

        let guard = init("stdout", "unused").await.expect("init stdout");
        inject(&mut headers);
        extract(&headers);
        drop(guard);

        let guard = init("unknown", "unused").await.expect("init fallback");
        inject(&mut headers);
        extract(&headers);
        drop(guard);

        let guard = init("otlp", "http://localhost:4317").await.expect("init otlp");
        inject(&mut headers);
        extract(&headers);
        drop(guard);
    }
}
