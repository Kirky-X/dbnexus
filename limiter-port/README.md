# dbnexus-limiter-port

[dbnexus](https://github.com/Kirky-X/dbnexus) 仓库的限流端口子 crate（目录 `dbnexus/limiter-port`）。

## 与父 crate 的关系

| 项 | 内容 |
|---|---|
| 所属仓库 | [dbnexus](https://github.com/Kirky-X/dbnexus)（[crates.io](https://crates.io/crates/dbnexus)） |
| 依赖方向 | 双向接缝：`dbnexus` → 本 crate（经端口做限流判定）；`limiteron` 侧另由 [`limiteron-dbnexus`](https://github.com/Kirky-X/limiteron/tree/main/integrations/limiteron-dbnexus) 实现本端口 |
| 是否 re-export | 否：端口 trait 由使用方直接引入 |
| 适用场景 | 需要为 dbnexus 提供自定义限流实现（外部 Redis、服务网格、自研算法）时，实现本 crate 的 `Limiter` 端口即可，无需改动 dbnexus 本体 |

## 为什么单独成 crate

dbnexus 与 limiteron 互为消费方（limiteron 可选依赖 dbnexus 作存储后端），Cargo 禁止包级循环依赖，故把限流判定接口抽为独立的极小 crate：仅 `async-trait` 一个常规依赖，不含任何驱动或存储实现。它是两条产品线之间唯一的无环接缝。

## 使用

```rust
use dbnexus_limiter_port::{Limiter, RateLimitDecision, RateLimitError};

struct MyLimiter;

#[async_trait::async_trait]
impl Limiter for MyLimiter {
    async fn check(&self, key: &str) -> Result<RateLimitDecision, RateLimitError> {
        // 自定义判定：allow() 放行，deny(retry_after) 拒绝并可携带 retry_after
        Ok(RateLimitDecision::allow())
    }
}
```

装配层通过 dbnexus 的 `RateLimitBackend::External` 注入实现。

## 版本与支持

- 版本经 `version.workspace = true` 随仓库统一，与 `dbnexus` 同号发布。
- 仅维护当前 minor 版本线，详见 [dbnexus 安全文档 · 支持的版本](https://github.com/Kirky-X/dbnexus/blob/main/docs/SECURITY.md#-支持的版本与维护策略)。

## 许可证

MIT
