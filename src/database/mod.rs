// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Database 模块
//!
//! 提供数据库连接管理、迁移、分片、图数据库等功能

pub mod graph;
#[cfg(feature = "migration")]
pub mod migration;
pub mod pool;

// 单文件模块
#[cfg(feature = "sharding")]
pub mod sharding;

// Re-exports
#[cfg(feature = "migration")]
pub use migration::MigrationExecutor;
#[cfg(feature = "migration")]
pub use migration::{
    Column, ColumnType, Index, Migration, MigrationFile, MigrationFileParser, MigrationHistory, MigrationVersion,
    Schema, SchemaDiffer, SqlGenerator, Table, TableChange,
};
pub use pool::{
    ConnectionPool, DatabaseConnection, DatabaseSession, DbConnection, DbPool, DbPoolBuilder, PoolStatus, Session,
};
pub use pool::{ConnectionTrait, TransactionTrait};
#[cfg(feature = "duckdb")]
pub use pool::{DuckDbConnection, DuckDbExecResult, DuckDbRow};
#[cfg(feature = "sharding")]
pub use sharding::{ShardConfig, ShardRouter, ShardingStrategy, create_strategy};

// 图数据库 re-exports
#[cfg(feature = "ladybug")]
pub use graph::ladybug_conn::LadybugConnection;
#[cfg(feature = "neo4j")]
pub use graph::neo4j_conn::Neo4jConnection;
pub use graph::{
    GraphConnection, GraphExecResult, GraphNode, GraphQueryResult, GraphRel, GraphRow, GraphTransaction, GraphValue,
};
