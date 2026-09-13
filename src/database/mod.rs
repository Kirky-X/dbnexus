// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Database 模块
//!
//! 提供数据库连接管理、迁移、分片、图数据库等功能

// 图数据库抽象层（始终可用：提供类型定义和 trait，具体实现在 ladybug/neo4j feature 后）
pub mod graph;
#[cfg(feature = "migration")]
pub mod migration;
pub mod pool;

// 单文件模块
#[cfg(feature = "sharding")]
pub mod sharding;

/// 副本路由模块（replica-routing feature）
#[cfg(feature = "replica-routing")]
pub mod replica;

/// 跨分片查询引擎（scatter-gather feature）
#[cfg(feature = "scatter-gather")]
pub mod scatter;

/// confers 配置热重载集成（config-confers feature）
#[cfg(feature = "config-confers")]
pub mod config_confers;

/// COPY 批量写入（copy feature；协议传输路径在 postgres 驱动组下启用）
#[cfg(feature = "copy")]
pub mod copy;

/// 泛型仓储
#[cfg(feature = "repository")]
pub mod repository;

/// 数据 API 网关雏形
#[cfg(feature = "data-api")]
pub mod data_api;

/// 查询 DSL 宏
#[cfg(feature = "query-dsl")]
pub mod query_dsl;

/// 实体事件总线 + Outbox
#[cfg(feature = "entity-events")]
pub mod entity_events;

/// 分布式事务 Saga 编排器（saga feature）
#[cfg(feature = "saga")]
pub mod saga;

// Re-exports
#[cfg(feature = "migration")]
pub use migration::MigrationExecutor;
#[cfg(feature = "migration")]
pub use migration::{
    Column, ColumnType, Index, Migration, MigrationFile, MigrationFileParser, MigrationHistory,
    MigrationVersion, Schema, SchemaDiffer, SqlGenerator, Table, TableChange,
};
pub use pool::{
    ConnectionPool, DatabaseConnection, DatabaseSession, DbConnection, DbPool, DbPoolBuilder,
    PoolStatus, Session,
};
pub use pool::{ConnectionTrait, TransactionTrait};
#[cfg(feature = "duckdb")]
pub use pool::{DuckDbConnection, DuckDbExecResult, DuckDbRow, DuckValue};
#[cfg(feature = "sharding")]
pub use sharding::{
    ConsistentHashStrategy, ShardConfig, ShardRouter, ShardingStrategy, create_strategy,
};

// Scatter-Gather re-exports
#[cfg(feature = "scatter-gather")]
pub use scatter::{
    AggregateFunction, AggregateValue, PartialFailurePolicy, ScatterGatherExecutor, ScatterResult,
    ShardError,
};

// Saga re-exports
#[cfg(all(feature = "saga", feature = "sql-parser"))]
pub use saga::DbSagaLog;
#[cfg(feature = "saga")]
pub use saga::{
    InMemorySagaLog, SagaAction, SagaError, SagaExecutionResult, SagaLog, SagaLogStore,
    SagaOrchestrator, SagaRecovery, SagaStatus, SagaStep, SagaStepLog,
};

// COPY 批量写入 re-exports
#[cfg(feature = "copy")]
pub use copy::{CopyFormat, CopyStatement, encode_copy_rows};

// 图数据库 re-exports
#[cfg(feature = "ladybug")]
pub use graph::ladybug_conn::LadybugConnection;
#[cfg(feature = "neo4j")]
pub use graph::neo4j_conn::Neo4jConnection;
pub use graph::{
    GraphConnection, GraphExecResult, GraphNode, GraphQueryResult, GraphRel, GraphRow,
    GraphTransaction, GraphValue,
};
