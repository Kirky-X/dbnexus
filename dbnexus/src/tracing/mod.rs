//! 分布式追踪模块
//!
//! 提供基于 OpenTelemetry 的分布式追踪功能。
//! 支持 OTLP 和标准输出导出器。

use opentelemetry::global;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{Config, TracerProvider};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry::KeyValue;
use std::collections::HashMap;

/// 追踪初始化结果
pub struct TracingGuard {
    _provider: TracerProvider,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        global::shutdown_tracer_provider();
    }
}

/// 初始化分布式追踪
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
    let resource = Resource::new(vec![
        KeyValue::new("service.name", "dbnexus"),
    ]);

    let config = Config::default().with_resource(resource);

    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint),
        )
        .with_trace_config(config)
        .install_simple()
        .map_err(|e| e.to_string())?;

    Ok(provider)
}

/// 使用标准输出初始化追踪
fn init_stdout() -> Result<TracerProvider, String> {
    let resource = Resource::new(vec![
        KeyValue::new("service.name", "dbnexus"),
    ]);

    let config = Config::default().with_resource(resource);

    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("stdout"),
        )
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