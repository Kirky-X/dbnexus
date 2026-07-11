// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Docker test utilities using testcontainers
//!
//! This module provides helper functions for setting up Docker-based test environments
//! using testcontainers for Redis and PostgreSQL.

use testcontainers::images::redis::Redis;
use testcontainers::images::postgres::Postgres;
use testcontainers::Container;

/// Creates a Redis testcontainer for testing
pub fn setup_redis_container() -> Container<'static, Redis> {
    testcontainers::run("redis:7-alpine")
        .expect("Failed to start Redis container")
}

/// Creates a PostgreSQL testcontainer for testing
pub fn setup_postgres_container() -> Container<'static, Postgres> {
    testcontainers::run("postgres:15-alpine")
        .with_env_var("POSTGRES_USER", "test")
        .with_env_var("POSTGRES_PASSWORD", "test")
        .with_env_var("POSTGRES_DB", "test")
        .expect("Failed to start PostgreSQL container")
}

/// Helper to get Redis connection URL from container
pub fn get_redis_url(container: &Container<'static, Redis>) -> String {
    let host_port = container.get_host_port_ipv4(6379);
    format!("redis://127.0.0.1:{}", host_port)
}

/// Helper to get PostgreSQL connection string from container
pub fn get_postgres_url(container: &Container<'static, Postgres>) -> String {
    let host_port = container.get_host_port_ipv4(5432);
    format!(
        "postgres://test:test@127.0.0.1:{}/test",
        host_port
    )
}
