// Copyright (c) 2026 Kirky.X
//
// Licensed under MIT License
// See LICENSE file in project root for full license information.

use sqlparser::ast::{AlterTableOperation, Statement};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

/// SQL逆向生成器
pub struct SqlReverser {
    dialect: sqlparser::dialect::GenericDialect,
}

impl SqlReverser {
    /// 创建新的SQL逆向生成器
    ///
    /// # Returns
    ///
    /// 新的 SqlReverser 实例
    ///
    /// # Example
    ///
    /// ```rust
    /// let reverser = SqlReverser::new();
    /// ```
    pub fn new() -> Self {
        Self {
            dialect: GenericDialect {},
        }
    }

    /// 逆向SQL语句
    ///
    /// # Arguments
    ///
    /// * `up_sql` - UP SQL语句
    ///
    /// # Returns
    ///
    /// - `Ok(String)` - 生成的DOWN SQL
    /// - `Err(String)` - 解析或转换失败
    ///
    /// # Example
    ///
    /// ```rust
    /// let reverser = SqlReverser::new();
    /// let up_sql = "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255));";
    /// let down_sql = reverser.reverse(up_sql).unwrap();
    /// ```
    pub fn reverse(&self, up_sql: &str) -> Result<String, String> {
        let statements = Parser::parse_sql(&self.dialect, up_sql).map_err(|e| format!("Failed to parse SQL: {}", e))?;

        let mut down_statements = Vec::new();

        for stmt in statements {
            let down_sql = self.reverse_statement(&stmt)?;
            down_statements.push(down_sql);
        }

        Ok(down_statements.join(";\n"))
    }

    /// 逆向单个SQL语句
    fn reverse_statement(&self, stmt: &Statement) -> Result<String, String> {
        match stmt {
            Statement::CreateTable { name, .. } => {
                let table_name = name.to_string();
                Ok(format!("DROP TABLE IF EXISTS {}", table_name))
            }
            Statement::AlterTable { name, operations, .. } => {
                let table_name = name.to_string();
                self.reverse_alter_table(&table_name, operations)
            }
            _ => Err(format!("Unsupported statement type for reversal: {:?}", stmt)),
        }
    }

    /// 逆向ALTER TABLE操作
    fn reverse_alter_table(&self, table_name: &str, operations: &[AlterTableOperation]) -> Result<String, String> {
        let mut down_operations = Vec::new();

        for op in operations {
            match op {
                AlterTableOperation::AddColumn { column_def, .. } => {
                    let column_name = &column_def.name.value;
                    down_operations.push(format!("ALTER TABLE {} DROP COLUMN {}", table_name, column_name));
                }
                AlterTableOperation::RenameColumn {
                    old_column_name,
                    new_column_name,
                    ..
                } => {
                    down_operations.push(format!(
                        "ALTER TABLE {} RENAME COLUMN {} TO {}",
                        table_name, old_column_name.value, new_column_name.value
                    ));
                }
                _ => {
                    return Err(format!("Unsupported ALTER operation: {:?}", op));
                }
            }
        }

        Ok(down_operations.join(";\n"))
    }
}

impl Default for SqlReverser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::SqlReverser;

    #[test]
    fn test_reverse_create_table() {
        let up_sql = "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255));";
        let reverser = SqlReverser::new();
        let down_sql = reverser.reverse(up_sql).unwrap();
        assert!(down_sql.contains("DROP TABLE"));
        assert!(down_sql.contains("users"));
    }

    #[test]
    fn test_reverse_add_column() {
        let up_sql = "ALTER TABLE users ADD COLUMN email VARCHAR(255);";
        let reverser = SqlReverser::new();
        let down_sql = reverser.reverse(up_sql).unwrap();
        assert!(down_sql.contains("DROP COLUMN"));
        assert!(down_sql.contains("email"));
    }
}
