// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! 分布式追踪模块
//!
//! 基于 OpenTelemetry + tracing-opentelemetry 桥接，提供 OTLP gRPC 导出能力。
//!
//! 使用 [`TracingGuard::init_with_otlp`] 初始化全局 tracer，返回的 guard 在 drop 时
//! 自动 flush 挂起的 span 并关闭 tracer provider（RAII 语义）。
//!
//! # 示例
//!
//! ```no_run
//! # #[cfg(feature = "tracing")] {
//! let _guard = dbnexus::TracingGuard::init_with_otlp("http://localhost:4317")
//!     .expect("tracing init failed");
//! // _guard drop 时自动 flush
//! # }
//! ```

use std::sync::OnceLock;

use thiserror::Error;

/// 全局初始化标记：tracing subscriber 一旦设置即不可重复（tracing-subscriber 全局限制）
static TRACING_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Tracing 初始化错误
#[derive(Debug, Error)]
pub enum TracingError {
    /// OTLP exporter 创建失败（端点格式错误、 tonic 配置异常等）
    #[error("OTLP exporter initialization failed: {0}")]
    ExporterInit(String),
    /// 全局 tracer provider 设置失败
    #[error("Tracer provider setup failed: {0}")]
    ProviderSetup(String),
    /// tracing 全局 subscriber 已设置（进程内只能初始化一次）
    #[error("Tracing already initialized: global subscriber can only be set once per process")]
    AlreadyInitialized,
    /// 全局 subscriber 设置失败
    #[error("Failed to set global subscriber: {0}")]
    SubscriberSetup(String),
}

/// RAII guard：drop 时调用 `opentelemetry::global::shutdown_tracer_provider()` flush 挂起 span
///
/// 通过 [`TracingGuard::init_with_otlp`] 创建。guard 本身不持有资源，仅作为 RAII 语义载体：
/// 提醒用户在程序退出前保持 guard 存活，以便 Drop 时触发全局 flush。
pub struct TracingGuard {
    _private: (),
}

impl TracingGuard {
    /// 初始化 OTLP gRPC exporter + 全局 tracing subscriber
    ///
    /// - 创建 tonic gRPC `SpanExporter` 指向 `endpoint`
    /// - 构建 `TracerProvider`（batch exporter + Tokio runtime + service.name=dbnexus）
    /// - 设置为全局 tracer provider
    /// - 注册 `tracing_opentelemetry::OpenTelemetryLayer` 到全局 subscriber
    ///
    /// # 运行时要求
    ///
    /// 此函数使用 `Tokio` batch exporter，**必须在 Tokio 1.x 运行时上下文中调用**
    ///（例如 `#[tokio::main]` 或 `#[tokio::test]` 函数内）。在非异步上下文中调用会 panic。
    ///
    /// # 重复初始化
    ///
    /// tracing 全局 subscriber 只能设置一次。第二次调用返回 [`TracingError::AlreadyInitialized`]，
    /// 不会 panic。
    ///
    /// # 错误
    ///
    /// - [`TracingError::ExporterInit`]: OTLP exporter 构建失败
    /// - [`TracingError::SubscriberSetup`]: `try_init` 失败（通常因已被其他代码设置）
    /// - [`TracingError::AlreadyInitialized`]: 本函数已在当前进程调用过一次
    pub fn init_with_otlp(endpoint: &str) -> Result<Self, TracingError> {
        // 幂等检查：已初始化则直接返回错误（不 panic）
        if TRACING_INITIALIZED.get().is_some() {
            return Err(TracingError::AlreadyInitialized);
        }

        use opentelemetry::global;
        use opentelemetry::trace::TracerProvider as OtelTracerProvider;
        use opentelemetry_sdk::runtime::Tokio;
        use opentelemetry_sdk::trace::TracerProvider;
        use opentelemetry_sdk::Resource;
        use opentelemetry_otlp::WithExportConfig;
        use tracing_opentelemetry::layer;
        use tracing_subscriber::layer::SubscriberExt;

        // 1. 创建 OTLP gRPC SpanExporter
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_protocol(opentelemetry_otlp::Protocol::Grpc)
            .build()
            .map_err(|e| TracingError::ExporterInit(e.to_string()))?;

        // 2. 构建 TracerProvider（batch exporter + Tokio runtime）
        let provider = TracerProvider::builder()
            .with_batch_exporter(exporter, Tokio)
            .with_resource(Resource::new(vec![opentelemetry::KeyValue::new(
                "service.name",
                "dbnexus",
            )]))
            .build();

        // 3. 从 provider 获取 SdkTracer（实现 PreSampledTracer，BoxedTracer 未实现）
        let tracer = OtelTracerProvider::tracer(&provider, "dbnexus");

        // 4. 设置全局 tracer provider
        global::set_tracer_provider(provider);

        // 5. 创建 OpenTelemetry tracing layer 并设置全局 subscriber
        let otel_layer = layer().with_tracer(tracer);
        let subscriber = tracing_subscriber::registry().with(otel_layer);
        tracing::subscriber::set_global_default(subscriber)
            .map_err(|e| TracingError::SubscriberSetup(e.to_string()))?;

        // 6. 标记已初始化（OnceLock::set 在已设置时返回 Err，此处 get 已检查所以安全）
        let _ = TRACING_INITIALIZED.set(());

        Ok(TracingGuard { _private: () })
    }
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        // flush 挂起 span 并关闭全局 tracer provider
        // opentelemetry 0.27: global::shutdown_tracer_provider() 同步 flush 后 drop provider
        // 多次调用安全（无 provider 时为 no-op）
        opentelemetry::global::shutdown_tracer_provider();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_error_display() {
        let err = TracingError::ExporterInit("connection refused".to_string());
        assert!(err.to_string().contains("OTLP exporter"));
        assert!(err.to_string().contains("connection refused"));

        let err = TracingError::AlreadyInitialized;
        assert!(err.to_string().contains("already initialized"));
    }

    #[test]
    fn test_tracing_guard_private_field() {
        // TracingGuard 只能通过 init_with_otlp 构造，_private 字段阻止外部直接构造
        // 此测试验证 guard 类型存在且 Send（可在 tokio 任务间传递）
        fn assert_send<T: Send>() {}
        assert_send::<TracingGuard>();
    }
}
