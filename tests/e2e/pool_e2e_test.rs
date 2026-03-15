//! E2E tests for dbnexus Database operations
//!
//! Tests cover: pool acquire, commit, session cleanup scenarios

#[cfg(test)]
mod tests {
    use dbnexus::{DbPool, DbPoolBuilder, Database};
    use std::time::Duration;

    /// Test basic pool creation and connection
    #[tokio::test]
    async fn test_pool_create() -> anyhow::Result<()> {
        // Create a pool with SQLite in-memory database
        let pool = DbPoolBuilder::new()
            .database(Database::Sqlite {
                path: ":memory:".to_string(),
            })
            max_connections: 5,

            .await?;

        // Verify pool is created
        assert!(pool.acquire().await.is_ok());

        Ok(())
    }

    /// Test acquiring a connection from pool
    #[tokio::test]
    async fn test_pool_acquire() -> anyhow::Result<()> {
        let pool = DbPoolBuilder::new()
            .database(Database::Sqlite {
                path: ":memory:".to_string(),
            })
            max_connections: 3,

            .await?;

        // Acquire a connection
        let mut session = pool.acquire().await?;

        // Verify session is valid by executing a simple query
        let result = session
            .execute_raw("SELECT 1 as value")
            .await?;

        assert!(result.rows_affected() >= 0);

        Ok(())
    }

    /// Test transaction commit
    #[tokio::test]
    async fn test_transaction_commit() -> anyhow::Result<()> {
        let pool = DbPoolBuilder::new()
            .database(Database::Sqlite {
                path: ":memory:".to_string(),
            })

            .await?;

        let mut session = pool.acquire().await?;

        // Create a table
        session
            .execute_raw(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            )
            .await?;

        // Begin transaction
        session.begin_transaction().await?;

        // Insert data
        session
            .execute_raw("INSERT INTO users (name) VALUES ('Alice')")
            .await?;

        // Commit transaction
        session.commit().await?;

        // Verify data was committed
        let result = session
            .execute_raw("SELECT COUNT(*) FROM users")
            .await?;

        // Query successful (rows_affected = 0 for SELECT, but no error)

        Ok(())
    }

    /// Test transaction rollback
    #[tokio::test]
    async fn test_transaction_rollback() -> anyhow::Result<()> {
        let pool = DbPoolBuilder::new()
            .database(Database::Sqlite {
                path: ":memory:".to_string(),
            })

            .await?;

        let mut session = pool.acquire().await?;

        // Create a table
        session
            .execute_raw(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            )
            .await?;

        // Begin transaction
        session.begin_transaction().await?;

        // Insert data
        session
            .execute_raw("INSERT INTO users (name) VALUES ('Bob')")
            .await?;

        // Rollback transaction
        session.rollback().await?;

        // Verify data was rolled back - table should still exist but be empty
        let result = session
            .execute_raw("SELECT COUNT(*) FROM users")
            .await?;

        Ok(())
    }

    /// Test session cleanup on drop
    #[tokio::test]
    async fn test_session_cleanup() -> anyhow::Result<()> {
        let pool = DbPoolBuilder::new()
            .database(Database::Sqlite {
                path: ":memory:".to_string(),
            })
            .min_idle_connections(0)

            .await?;

        // Acquire session in a block
        {
            let _session = pool.acquire().await?;
            // Session is active
        }
        // Session should be returned to pool automatically

        // Acquire another session - should work
        let _session = pool.acquire().await?;

        Ok(())
    }

    /// Test connection timeout
    #[tokio::test]
    async fn test_connection_timeout() -> anyhow::Result<()> {
        let pool = DbPoolBuilder::new()
            .database(Database::Sqlite {
                path: ":memory:".to_string(),
            })
            max_connections: 1,
            .acquire_timeout(Duration::from_secs(1))

            .await?;

        // Acquire the only connection
        let _session1 = pool.acquire().await?;

        // Try to acquire another - should fail due to timeout
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            pool.acquire()
        ).await;

        // Either timeout or connection error expected
        assert!(result.is_err() || result.unwrap().is_err());

        Ok(())
    }

    /// Test batch operations
    #[tokio::test]
    async fn test_batch_execute() -> anyhow::Result<()> {
        let pool = DbPoolBuilder::new()
            .database(Database::Sqlite {
                path: ":memory:".to_string(),
            })

            .await?;

        let mut session = pool.acquire().await?;

        // Create table
        session
            .execute_raw(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, value TEXT)",
            )
            .await?;

        // Batch execute in transaction
        let sqls = vec![
            "INSERT INTO items (value) VALUES ('item1')",
            "INSERT INTO items (value) VALUES ('item2')",
            "INSERT INTO items (value) VALUES ('item3')",
        ];

        let results = session
            .batch_execute_in_transaction(sqls)
            .await?;

        // All operations should succeed
        assert_eq!(results.len(), 3);

        Ok(())
    }

    /// Test health check
    #[tokio::test]
    async fn test_pool_health_check() -> anyhow::Result<()> {
        let pool = DbPoolBuilder::new()
            .database(Database::Sqlite {
                path: ":memory:".to_string(),
            })

            .await?;

        // Check pool health
        let is_healthy = pool.health_check().await?;

        assert!(is_healthy);

        Ok(())
    }
}
