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

/// RAII guard：drop 时调用 `SdkTracerProvider::shutdown()` flush 挂起 span
///
/// 通过 [`TracingGuard::init_with_otlp`] 创建。guard 持有 `SdkTracerProvider` 的克隆，
/// 在 Drop 时调用其 `shutdown()` 方法触发 batch flush 并关闭后台导出线程。
///
/// opentelemetry 0.32 移除了 `global::shutdown_tracer_provider()`，
/// shutdown 责任由 provider 自身承担（`SdkTracerProvider::shutdown`）。
/// 由于 `SdkTracerProvider` 内部用 `Arc` 共享，全局也持有一份克隆，
/// 但 shutdown 通过共享的 `is_shutdown` 标志位传播，因此本 guard 调用 shutdown
/// 会同时让全局 provider 进入已关闭状态。
pub struct TracingGuard {
    provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl TracingGuard {
    /// 初始化 OTLP gRPC exporter + 全局 tracing subscriber
    ///
    /// - 创建 tonic gRPC `SpanExporter` 指向 `endpoint`
    /// - 构建 `SdkTracerProvider`（batch span processor + service.name=dbnexus）
    /// - 设置为全局 tracer provider
    /// - 注册 `tracing_opentelemetry::OpenTelemetryLayer` 到全局 subscriber
    ///
    /// # 运行时说明
    ///
    /// opentelemetry_sdk 0.32 的 `BatchSpanProcessor` 使用独立 OS 线程进行批处理，
    /// 不再依赖 Tokio runtime 参数。但 tonic gRPC 客户端在建立 channel 时
    /// 仍需 reactor，建议在 Tokio 1.x 运行时上下文中调用
    ///（例如 `#[tokio::main]` 或 `#[tokio::test]` 函数内）。
    ///
    /// # 重复初始化
    ///
    /// tracing 全局 subscriber 只能设置一次。第二次调用返回 [`TracingError::AlreadyInitialized`]，
    /// 不会 panic。
    ///
    /// # 错误
    ///
    /// - [`TracingError::ExporterInit`] — OTLP exporter 构建失败
    /// - [`TracingError::SubscriberSetup`] — `try_init` 失败（通常因已被其他代码设置）
    /// - [`TracingError::AlreadyInitialized`] — 本函数已在当前进程调用过一次
    pub fn init_with_otlp(endpoint: &str) -> Result<Self, TracingError> {
        // 幂等检查：已初始化则直接返回错误（不 panic）
        if TRACING_INITIALIZED.get().is_some() {
            return Err(TracingError::AlreadyInitialized);
        }

        use opentelemetry::global;
        use opentelemetry::trace::TracerProvider as OtelTracerProvider;
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::Resource;
        use opentelemetry_sdk::trace::SdkTracerProvider;
        use tracing_opentelemetry::layer;
        use tracing_subscriber::layer::SubscriberExt;

        // 1. 创建 OTLP gRPC SpanExporter
        //    opentelemetry-otlp 0.32: with_tonic() 隐含 Protocol::Grpc，无需显式指定
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .map_err(|e| TracingError::ExporterInit(e.to_string()))?;

        // 2. 构建 SdkTracerProvider（batch span processor 使用独立 OS 线程，无需 runtime 参数）
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(Resource::builder().with_service_name("dbnexus").build())
            .build();

        // 3. 从 provider 获取 SdkTracer（实现 PreSampledTracer，BoxedTracer 未实现）
        let tracer = OtelTracerProvider::tracer(&provider, "dbnexus");

        // 4. 设置全局 tracer provider（传入 provider 的克隆，本 guard 保留另一克隆用于 shutdown）
        global::set_tracer_provider(provider.clone());

        // 5. 创建 OpenTelemetry tracing layer 并设置全局 subscriber
        let otel_layer = layer().with_tracer(tracer);
        let subscriber = tracing_subscriber::registry().with(otel_layer);
        tracing::subscriber::set_global_default(subscriber)
            .map_err(|e| TracingError::SubscriberSetup(e.to_string()))?;

        // 6. 标记已初始化（OnceLock::set 在已设置时返回 Err，此处 get 已检查所以安全）
        let _ = TRACING_INITIALIZED.set(());

        Ok(TracingGuard {
            provider: Some(provider),
        })
    }
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        // flush 挂起 span 并关闭 tracer provider
        // opentelemetry 0.32: 调用 SdkTracerProvider::shutdown() 同步 flush 后置位 is_shutdown
        // shutdown 通过共享的 inner.is_shutdown 标志位传播，全局 provider 的克隆也会进入已关闭状态
        // 多次调用安全（已 shutdown 时返回 AlreadyShutdown 错误，此处忽略）
        if let Some(provider) = self.provider.take() {
            let _ = provider.shutdown();
        }
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
        // TracingGuard 只能通过 init_with_otlp 构造，provider 字段为 Option<SdkTracerProvider>
        // 此测试验证 guard 类型存在且 Send（可在 tokio 任务间传递）
        fn assert_send<T: Send>() {}
        assert_send::<TracingGuard>();
    }
}
