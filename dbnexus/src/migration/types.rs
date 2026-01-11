pub enum TableChange {
    /// 新增表
    CreateTable(Table),
    /// 删除表
    ///
    /// # Fields
    ///
    /// * `table_name` - 被删除的表名
    DropTable {
        /// 被删除的表名
        table_name: String,
    },
    /// 修改表
    ///
    /// 被修改的表名
    AlterTable {
        /// 表名
        table_name: String,
        /// 列变更列表
        column_changes: Vec<ColumnChange>,
        /// 新增的列
        added_columns: Vec<Column>,
        /// 删除的列名列表
        removed_columns: Vec<String>,
        /// 新增的索引
        added_indexes: Vec<Index>,
        /// 删除的索引名列表
        removed_indexes: Vec<String>,
        /// 新增的外键
        added_foreign_keys: Vec<ForeignKey>,
        /// 删除的外键名列表
        removed_foreign_keys: Vec<String>,
    },
}

/// 列变更类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnChange {
    /// 列类型变更
    ///
    /// 变更的列名
    TypeChanged {
        /// 列名
        column_name: String,
        /// 旧的类型
        old_type: ColumnType,
        /// 新的类型
        new_type: ColumnType,
    },
    /// 列可空性变更
    ///
    /// 变更的列名和新的可空性
    NullabilityChanged {
        /// 列名
        column_name: String,
        /// 旧的可空性
        old_nullable: bool,
        /// 新的可空性
        new_nullable: bool,
    },
    /// 添加默认值
    ///
    /// 变更的列名和新的默认值
    DefaultChanged {
        /// 列名
        column_name: String,
        /// 旧的默认值
        old_default: Option<String>,
        /// 新的默认值
        new_default: Option<String>,
    },
}

/// Migration 变更
#[derive(Debug, Clone)]
