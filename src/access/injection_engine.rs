// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 统一注入检测引擎（T417）
//!
//! 此前注入检测规则散落在三处独立实现中：
//! 1. `sql_parser::contains_sql_injection` 的关系型静态模式表；
//! 2. `sql_parser::contains_variables` 的动态 SQL 变量正则；
//! 3. `session::validate_cypher_safety` 的图查询危险过程黑名单
//!    与 `ddl_guard::FORBIDDEN_PATTERNS` 的 DDL 禁用模式。
//!
//! 本模块将全部规则合并为单一注册表（去重后），按 [`RuleCategory`] 分类，
//! 由各调用方的方言管线（预处理）复用同一匹配器：
//! - 关系型管线（NFKC 规范化 → 块注释标记 → 去字符串 → 去块注释 → 大写）；
//! - DDL 管线（trim → 大写）；
//! - 图管线（原样小写）。
//!
//! # 去重记录（可证明等价）
//!
//! 合并时按"子串包含即冗余"剪枝——若模式 B 是模式 A 的子串，则包含 A 的输入
//! 必然包含 B，A 永远不会独有命中，可安全删除。剪掉的 4 条关系型模式：
//! - `"; EXECUTE"`（被 `"; EXEC"` 包含）
//! - `"EXEC xp_"`、`"EXECUTE xp_"`（被 `" xp_"` 包含）
//! - `"ORDER BY 1#"`（被 `"#"` 包含）
//!
//! 误报对比见模块底部 `parity` 测试：引擎与合并前遗留规则表在攻击语料与
//! 良性语料上的判定完全一致（保留既有误报轮廓，未扩大也未缩小）。

use std::sync::LazyLock;

/// 注入规则类别（T417：统一规则集的分类维度）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleCategory {
    /// UNION 注入（UNION SELECT 等）
    Union,
    /// 布尔盲注（OR 1=1 等）
    BooleanBlind,
    /// 时间盲注（SLEEP/PG_SLEEP/WAITFOR 等）
    TimeBlind,
    /// 动态 SQL 执行（EXEC/XP_CMDSHELL 等）
    DynamicExec,
    /// 文件操作（LOAD_FILE/INTO OUTFILE 等）
    FileOps,
    /// 信息泄露（INFORMATION_SCHEMA/系统表等）
    InfoLeak,
    /// 编码绕过（CHAR/CHR/CONCAT/0X 等）
    Encoding,
    /// 堆叠查询（; DROP 等）
    Stacked,
    /// 注释注入（--、#、块注释标记）
    Comment,
    /// 其他危险模式（HAVING 1=1、EXTRACTVALUE 等）
    Other,
    /// 动态 SQL 变量（@var/:var/${var} 等，关系型管线专用正则）
    #[cfg(feature = "sql-parser")]
    DynamicVariable,
    /// DDL 禁用模式（DROP DATABASE/DROP ALL，DDL 管线）
    DdlForbidden,
    /// 图数据库危险过程（CALL apoc. 等，图管线）
    GraphProcedure,
}

/// 统一注入规则（T417）
#[derive(Debug)]
pub struct InjectionRule {
    /// 稳定规则 ID（审计/日志引用）
    pub id: &'static str,
    /// 规则类别
    pub category: RuleCategory,
    /// 匹配模式（子串匹配；关系型管线对大写文本匹配）
    pub pattern: &'static str,
}

/// 统一注入检测引擎
///
/// 持有去重后的全部规则；匹配器纯同步无状态，全局单例经 [`InjectionEngine::global`]。
pub struct InjectionEngine {
    rules: Vec<InjectionRule>,
    /// 动态 SQL 变量正则（sql-parser feature；`?` 占位符不算危险变量）
    #[cfg(feature = "sql-parser")]
    variable_regexes: Vec<regex::Regex>,
}

/// 全局引擎：合并三处遗留规则表并完成去重剪枝
static GLOBAL_ENGINE: LazyLock<InjectionEngine> = LazyLock::new(InjectionEngine::build_global);

impl InjectionEngine {
    /// 全局引擎单例
    pub fn global() -> &'static Self {
        &GLOBAL_ENGINE
    }

    /// 注册表规模（去重后），供诊断与测试
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// 对预处理的文本做子串扫描，返回命中的规则
    ///
    /// `prepared` 必须已按调用方方言管线预处理（大写化等）；
    /// 仅匹配 `categories` 中列出的类别。
    pub fn scan(&self, prepared: &str, categories: &[RuleCategory]) -> Vec<&InjectionRule> {
        self.rules
            .iter()
            .filter(|rule| {
                categories.contains(&rule.category) && prepared.contains(rule.pattern)
            })
            .collect()
    }

    /// 关系型管线：完整预处理后扫描（sql-parser feature）
    ///
    /// 语义与合并前 `contains_sql_injection` 完全一致：
    /// NFKC 规范化 → 块注释标记检查（在移除前检测）→ 移除字符串字面量
    /// → 移除块注释 → 大写 → 子串扫描。
    #[cfg(feature = "sql-parser")]
    pub fn scan_relational(&self, sql: &str) -> Vec<&InjectionRule> {
        use crate::access::sql_parser::{normalize_unicode, remove_string_literals, strip_block_comments};

        // 第一步：Unicode 规范化（NFKC），防止 Unicode 绕过
        let normalized = normalize_unicode(sql);

        // 第二步：块注释标记在移除前检测（注释本身即为注入迹象）
        if normalized.contains("/*") {
            return self
                .rules
                .iter()
                .filter(|rule| rule.id == "comment.block_marker")
                .collect();
        }

        // 第三至四步：移除字符串字面量与块注释后大写
        let without_strings = remove_string_literals(&normalized);
        let without_comments = strip_block_comments(&without_strings);
        let prepared = without_comments.to_uppercase();

        self.scan(&prepared, &[
            RuleCategory::Union,
            RuleCategory::BooleanBlind,
            RuleCategory::TimeBlind,
            RuleCategory::DynamicExec,
            RuleCategory::FileOps,
            RuleCategory::InfoLeak,
            RuleCategory::Encoding,
            RuleCategory::Stacked,
            RuleCategory::Comment,
            RuleCategory::Other,
        ])
    }

    /// 关系型管线布尔口径（等价合并前的 `contains_sql_injection`）
    #[cfg(feature = "sql-parser")]
    pub fn is_suspicious_relational(&self, sql: &str) -> bool {
        !self.scan_relational(sql).is_empty()
    }

    /// 动态 SQL 变量检测（sql-parser feature；等价合并前的 `contains_variables`）
    ///
    /// `?` prepared-statement 占位符不算危险变量：它是参数绑定的安全形态。
    #[cfg(feature = "sql-parser")]
    pub fn has_dynamic_variables(&self, sql: &str) -> bool {
        use crate::access::sql_parser::remove_string_literals;

        // 先移除字符串字面量，避免误报
        let without_strings = remove_string_literals(sql);
        self.variable_regexes
            .iter()
            .any(|re| re.is_match(&without_strings))
    }

    /// DDL 管线：trim + 大写后扫描禁用模式
    ///
    /// 语义与合并前 `ddl_guard::FORBIDDEN_PATTERNS` 检查一致。
    pub fn scan_ddl(&self, sql: &str) -> Vec<&InjectionRule> {
        let prepared = sql.trim().to_uppercase();
        self.scan(&prepared, &[RuleCategory::DdlForbidden])
    }

    /// 图管线：原样小写后扫描危险过程与块注释
    ///
    /// 语义与合并前 `validate_cypher_safety` 的第 4/5 步一致
    /// （不做字符串剥离，图查询无对应预处理）。
    pub fn scan_graph(&self, cypher: &str) -> Vec<&InjectionRule> {
        let prepared = cypher.to_ascii_lowercase();
        self.scan(&prepared, &[RuleCategory::Comment, RuleCategory::GraphProcedure])
            .into_iter()
            .filter(|rule| rule.pattern.starts_with("call ") || rule.pattern == "/*" || rule.pattern == "*/")
            .collect()
    }

    /// 构建全局规则表（合并 + 去重剪枝）
    fn build_global() -> Self {
        // --- 关系型管线规则（来源：sql_parser::INJECTION_PATTERNS，去重后） ---
        const RELATIONAL: &[(&str, RuleCategory, &str)] = &[
            // === UNION 注入 ===
            // （"UNION ALL SELECT"/"UNION DISTINCT SELECT" 与 "UNION SELECT" 无子串
            //   包含关系，各自独立保留）
            ("union.select", RuleCategory::Union, "UNION SELECT"),
            ("union.all_select", RuleCategory::Union, "UNION ALL SELECT"),
            ("union.distinct_select", RuleCategory::Union, "UNION DISTINCT SELECT"),
            // === 布尔盲注 ===
            ("bool.or_1eq1", RuleCategory::BooleanBlind, " OR 1=1"),
            ("bool.or_1sp1", RuleCategory::BooleanBlind, " OR 1 =1"),
            ("bool.or_1sp_eq1", RuleCategory::BooleanBlind, " OR 1= 1"),
            ("bool.or_1sp_eq_sp1", RuleCategory::BooleanBlind, " OR 1 = 1"),
            ("bool.or_true", RuleCategory::BooleanBlind, " OR TRUE"),
            ("bool.or_false", RuleCategory::BooleanBlind, " OR FALSE"),
            ("bool.and_1eq1", RuleCategory::BooleanBlind, " AND 1=1"),
            ("bool.and_true", RuleCategory::BooleanBlind, " AND TRUE"),
            ("bool.and_false", RuleCategory::BooleanBlind, " AND FALSE"),
            // === 时间盲注 - MySQL ===
            ("time.sleep", RuleCategory::TimeBlind, "SLEEP("),
            ("time.benchmark", RuleCategory::TimeBlind, "BENCHMARK("),
            // === 时间盲注 - PostgreSQL ===
            ("time.pg_sleep", RuleCategory::TimeBlind, "PG_SLEEP("),
            ("time.pg_sleep_for", RuleCategory::TimeBlind, "PG_SLEEP_FOR("),
            ("time.pg_sleep_until", RuleCategory::TimeBlind, "PG_SLEEP_UNTIL("),
            // === 时间盲注 - SQL Server ===
            ("time.waitfor_delay", RuleCategory::TimeBlind, "WAITFOR DELAY"),
            ("time.waitfor_time", RuleCategory::TimeBlind, "WAITFOR TIME"),
            // === 时间盲注 - Oracle ===
            ("time.dbms_pipe", RuleCategory::TimeBlind, "DBMS_PIPE.RECEIVE_MESSAGE("),
            ("time.dbms_lock", RuleCategory::TimeBlind, "DBMS_LOCK.SLEEP("),
            // === 动态 SQL 执行 ===
            // （"; EXECUTE" 被 "; EXEC" 包含；"EXEC xp_"/"EXECUTE xp_" 被 " xp_"
            //   包含——三条冗余模式已在去重中剪除，见模块文档）
            ("exec.exec", RuleCategory::DynamicExec, "EXEC("),
            ("exec.execute", RuleCategory::DynamicExec, "EXECUTE("),
            ("exec.sp_executesql", RuleCategory::DynamicExec, "SP_EXECUTESQL"),
            ("exec.xp_cmdshell", RuleCategory::DynamicExec, "XP_CMDSHELL"),
            ("exec.xp_generic", RuleCategory::DynamicExec, " xp_"),
            // === 文件操作 ===
            ("file.load_file", RuleCategory::FileOps, "LOAD_FILE("),
            ("file.into_outfile", RuleCategory::FileOps, "INTO OUTFILE"),
            ("file.into_dumpfile", RuleCategory::FileOps, "INTO DUMPFILE"),
            // === 信息泄露 ===
            ("info.information_schema", RuleCategory::InfoLeak, "INFORMATION_SCHEMA"),
            ("info.sysobjects", RuleCategory::InfoLeak, "SYSOBJECTS"),
            ("info.syscolumns", RuleCategory::InfoLeak, "SYSCOLUMNS"),
            ("info.sys_tables", RuleCategory::InfoLeak, "SYS.TABLES"),
            ("info.sys_columns", RuleCategory::InfoLeak, "SYS.COLUMNS"),
            ("info.sys_databases", RuleCategory::InfoLeak, "SYS.DATABASES"),
            ("info.mysql_user", RuleCategory::InfoLeak, "MYSQL.USER"),
            ("info.pg_user", RuleCategory::InfoLeak, "PG_USER"),
            ("info.pg_shadow", RuleCategory::InfoLeak, "PG_SHADOW"),
            ("info.all_tables", RuleCategory::InfoLeak, "ALL_TABLES"),
            ("info.all_columns", RuleCategory::InfoLeak, "ALL_COLUMNS"),
            ("info.all_tab_columns", RuleCategory::InfoLeak, "ALL_TAB_COLUMNS"),
            ("info.user_tables", RuleCategory::InfoLeak, "USER_TABLES"),
            ("info.user_tab_columns", RuleCategory::InfoLeak, "USER_TAB_COLUMNS"),
            // === 编码绕过 ===
            ("enc.char", RuleCategory::Encoding, "CHAR("),
            ("enc.chr", RuleCategory::Encoding, "CHR("),
            ("enc.concat", RuleCategory::Encoding, "CONCAT("),
            ("enc.concat_ws", RuleCategory::Encoding, "CONCAT_WS("),
            ("enc.hex_prefix", RuleCategory::Encoding, "0X"),
            // === 堆叠查询 ===
            // （"; EXECUTE" 剪除，见上）
            ("stack.drop", RuleCategory::Stacked, "; DROP"),
            ("stack.delete", RuleCategory::Stacked, "; DELETE"),
            ("stack.update", RuleCategory::Stacked, "; UPDATE"),
            ("stack.insert", RuleCategory::Stacked, "; INSERT"),
            ("stack.truncate", RuleCategory::Stacked, "; TRUNCATE"),
            ("stack.alter", RuleCategory::Stacked, "; ALTER"),
            ("stack.create", RuleCategory::Stacked, "; CREATE"),
            ("stack.exec", RuleCategory::Stacked, "; EXEC"),
            // === 注释注入 ===
            ("comment.dash_space", RuleCategory::Comment, "-- "),
            ("comment.dash_plus", RuleCategory::Comment, "--+"),
            ("comment.hash", RuleCategory::Comment, "#"),
            // === 其他危险模式 ===
            // （"ORDER BY 1#" 被 "#" 包含，剪除；"ORDER BY 1--" 无尾随空格，
            //   与 "-- " 无包含关系，保留）
            ("other.having_tautology", RuleCategory::Other, "HAVING 1=1"),
            ("other.order_by_dash", RuleCategory::Other, "ORDER BY 1--"),
            ("other.procedure_analyse", RuleCategory::Other, "PROCEDURE ANALYSE("),
            ("other.extractvalue", RuleCategory::Other, "EXTRACTVALUE("),
            ("other.updatexml", RuleCategory::Other, "UPDATEXML("),
            ("other.xmltype", RuleCategory::Other, "XMLTYPE("),
            ("other.utl_http", RuleCategory::Other, "UTL_HTTP.REQUEST("),
            ("other.utl_inaddr_host", RuleCategory::Other, "UTL_INADDR.GET_HOST_ADDRESS("),
            ("other.utl_inaddr_name", RuleCategory::Other, "UTL_INADDR.GET_HOST_NAME("),
        ];

        // --- DDL 管线规则（来源：ddl_guard::FORBIDDEN_PATTERNS） ---
        const DDL: &[(&str, RuleCategory, &str)] = &[
            ("ddl.drop_database", RuleCategory::DdlForbidden, "DROP DATABASE"),
            ("ddl.drop_all", RuleCategory::DdlForbidden, "DROP ALL"),
        ];

        // --- 图管线规则（来源：validate_cypher_safety 第 4/5 步） ---
        const GRAPH: &[(&str, RuleCategory, &str)] = &[
            // 块注释标记为关系型与图管线共享的唯一 "/*" 规则
            // （关系型管线经 scan_relational 的早退分支消费，见模块文档）
            ("comment.block_marker", RuleCategory::Comment, "/*"),
            ("graph.block_comment_close", RuleCategory::Comment, "*/"),
            ("graph.call_apoc", RuleCategory::GraphProcedure, "call apoc."),
            ("graph.call_dbms", RuleCategory::GraphProcedure, "call dbms."),
            ("graph.call_db", RuleCategory::GraphProcedure, "call db."),
            ("graph.call_tx", RuleCategory::GraphProcedure, "call tx."),
        ];

        let mut rules: Vec<InjectionRule> = RELATIONAL
            .iter()
            .chain(DDL.iter())
            .chain(GRAPH.iter())
            .map(|(id, category, pattern)| InjectionRule {
                id,
                category: *category,
                pattern,
            })
            .collect();

        // 防御性去重：精确重复（同 pattern 同类别）只保留首个
        let mut seen: std::collections::HashSet<(&str, &str)> = std::collections::HashSet::new();
        rules.retain(|rule| seen.insert((rule.pattern, category_key(&rule.category))));

        Self {
            rules,
            #[cfg(feature = "sql-parser")]
            variable_regexes: vec![
                // 命名参数：@variable
                regex::Regex::new(r"@[\w]+").expect("Regex pattern should be valid"),
                // 命名参数：:variable
                regex::Regex::new(r":[a-zA-Z_][\w]*").expect("Regex pattern should be valid"),
                // Shell/PHP 变量：$variable、${variable}
                regex::Regex::new(r"\$\{?[\w]+\}?").expect("Regex pattern should be valid"),
                // 百分号编码参数：%variable%
                regex::Regex::new(r"%[\w]+%").expect("Regex pattern should be valid"),
                // 可能用于绕过过滤的十六进制字面量
                regex::Regex::new(r"0x[0-9A-Fa-f]+").expect("Regex pattern should be valid"),
                // 注意：`?` prepared-statement 占位符不算危险变量（参数绑定的安全形态）
            ],
        }
    }
}

/// RuleCategory 的比较键（无 DynamicVariable 的 cfg 组合下 Hash 可用性一致）
fn category_key(category: &RuleCategory) -> &'static str {
    match category {
        RuleCategory::Union => "union",
        RuleCategory::BooleanBlind => "bool",
        RuleCategory::TimeBlind => "time",
        RuleCategory::DynamicExec => "exec",
        RuleCategory::FileOps => "file",
        RuleCategory::InfoLeak => "info",
        RuleCategory::Encoding => "enc",
        RuleCategory::Stacked => "stack",
        RuleCategory::Comment => "comment",
        RuleCategory::Other => "other",
        #[cfg(feature = "sql-parser")]
        RuleCategory::DynamicVariable => "dynvar",
        RuleCategory::DdlForbidden => "ddl",
        RuleCategory::GraphProcedure => "graph",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合并前的遗留关系型规则表（去重前原样副本）——用于误报对比（parity）测试
    const LEGACY_RELATIONAL: &[&str] = &[
        "UNION SELECT",
        "UNION ALL SELECT",
        "UNION DISTINCT SELECT",
        " OR 1=1",
        " OR 1 =1",
        " OR 1= 1",
        " OR 1 = 1",
        " OR TRUE",
        " OR FALSE",
        " AND 1=1",
        " AND TRUE",
        " AND FALSE",
        "SLEEP(",
        "BENCHMARK(",
        "PG_SLEEP(",
        "PG_SLEEP_FOR(",
        "PG_SLEEP_UNTIL(",
        "WAITFOR DELAY",
        "WAITFOR TIME",
        "DBMS_PIPE.RECEIVE_MESSAGE(",
        "DBMS_LOCK.SLEEP(",
        "EXEC(",
        "EXECUTE(",
        "SP_EXECUTESQL",
        "XP_CMDSHELL",
        " xp_",
        "EXEC xp_",
        "EXECUTE xp_",
        "LOAD_FILE(",
        "INTO OUTFILE",
        "INTO DUMPFILE",
        "INFORMATION_SCHEMA",
        "SYSOBJECTS",
        "SYSCOLUMNS",
        "SYS.TABLES",
        "SYS.COLUMNS",
        "SYS.DATABASES",
        "MYSQL.USER",
        "PG_USER",
        "PG_SHADOW",
        "ALL_TABLES",
        "ALL_COLUMNS",
        "ALL_TAB_COLUMNS",
        "USER_TABLES",
        "USER_TAB_COLUMNS",
        "CHAR(",
        "CHR(",
        "CONCAT(",
        "CONCAT_WS(",
        "0X",
        "; DROP",
        "; DELETE",
        "; UPDATE",
        "; INSERT",
        "; TRUNCATE",
        "; ALTER",
        "; CREATE",
        "; EXEC",
        "; EXECUTE",
        "-- ",
        "--+",
        "#",
        "HAVING 1=1",
        "ORDER BY 1--",
        "ORDER BY 1#",
        "PROCEDURE ANALYSE(",
        "EXTRACTVALUE(",
        "UPDATEXML(",
        "XMLTYPE(",
        "UTL_HTTP.REQUEST(",
        "UTL_INADDR.GET_HOST_ADDRESS(",
        "UTL_INADDR.GET_HOST_NAME(",
    ];

    /// 遗留匹配口径：任意模式命中即真
    fn legacy_contains(prepared_upper: &str) -> bool {
        LEGACY_RELATIONAL
            .iter()
            .any(|pattern| prepared_upper.contains(pattern))
    }

    /// 引擎匹配口径（关系型类别全集）
    fn engine_contains(prepared_upper: &str) -> bool {
        !InjectionEngine::global().scan(
            prepared_upper,
            &[
                RuleCategory::Union,
                RuleCategory::BooleanBlind,
                RuleCategory::TimeBlind,
                RuleCategory::DynamicExec,
                RuleCategory::FileOps,
                RuleCategory::InfoLeak,
                RuleCategory::Encoding,
                RuleCategory::Stacked,
                RuleCategory::Comment,
                RuleCategory::Other,
            ],
        ).is_empty()
    }

    /// 误报对比（攻击语料 + 良性语料）：引擎与遗留表判定逐一一致
    #[test]
    fn test_parity_with_legacy_rule_set() {
        let attack_corpus = [
            "SELECT * FROM USERS WHERE ID = 1 UNION SELECT PASSWORD FROM CREDENTIALS",
            "SELECT 1 OR 1=1",
            "SELECT 1 AND TRUE",
            "SELECT SLEEP(10)",
            "SELECT PG_SLEEP(5)",
            "WAITFOR DELAY 0:0:10",
            "EXEC XP_CMDSHELL DIR",
            "EXECUTE SP_EXECUTESQL X",
            "SELECT USER_X FROM T", // 含 " xp_" 形态
            "SELECT LOAD_FILE('/ETC/PASSWD')",
            "SELECT * INTO OUTFILE '/TMP/X'",
            "SELECT * FROM INFORMATION_SCHEMA.TABLES",
            "SELECT CHR(65)",
            "SELECT CONCAT(A, B) FROM T",
            "SELECT 1; DROP TABLE USERS",
            "SELECT 1 -- COMMENT",
            "SELECT 1 #COMMENT",
            "SELECT * FROM T HAVING 1=1",
            "SELECT EXTRACTVALUE(1, CONCAT(0X5C))",
        ];
        let benign_corpus = [
            "SELECT ID, NAME FROM USERS WHERE ID = ?",
            "SELECT COUNT(*) AS C FROM ORDERS WHERE STATUS = PAID",
            "INSERT INTO LOGS (LEVEL, MESSAGE) VALUES (INFO, OK)",
            "UPDATE USERS SET LAST_SEEN = NOW() WHERE ID = 42",
            "SELECT NAME FROM CATEGORIES WHERE PARENT_ID IS NULL",
            "SELECT A FROM T WHERE B IN (1, 2, 3)",
        ];

        let corpora: [&[&str]; 2] = [&attack_corpus, &benign_corpus];
        for corpus in corpora {
            for input in corpus {
                assert_eq!(
                    engine_contains(input),
                    legacy_contains(input),
                    "引擎与遗留规则表判定不一致: {input}"
                );
            }
        }

        // 块注释标记走管线早退分支（与遗留 contains_sql_injection 的前置检查同位），
        // 不参与匹配阶段语料对比（scan_relational 仅在 sql-parser 下编译）
        #[cfg(feature = "sql-parser")]
        assert!(
            !InjectionEngine::global()
                .scan_relational("SELECT /* HIDDEN */ 1")
                .is_empty()
        );
    }

    /// 去重剪枝正确性：被包含模式不存在独立命中
    #[test]
    fn test_dedup_pruned_rules_have_no_unique_matches() {
        let engine = InjectionEngine::global();
        // 关系型类别规则数 = 遗留表 72 条 - 剪除的 4 条冗余
        // （"; EXECUTE"/"EXEC xp_"/"EXECUTE xp_"/"ORDER BY 1#"）
        // 共享的块注释标记规则（关系型早退分支与图管线复用）与图管线专属规则
        // （graph.* 前缀）不计入关系型匹配口径
        let relational_count = engine
            .rules
            .iter()
            .filter(|rule| {
                !rule.id.starts_with("graph.") && rule.id != "comment.block_marker"
            })
            .filter(|rule| {
                matches!(
                    rule.category,
                    RuleCategory::Union
                        | RuleCategory::BooleanBlind
                        | RuleCategory::TimeBlind
                        | RuleCategory::DynamicExec
                        | RuleCategory::FileOps
                        | RuleCategory::InfoLeak
                        | RuleCategory::Encoding
                        | RuleCategory::Stacked
                        | RuleCategory::Comment
                        | RuleCategory::Other
                )
            })
            .count();
        assert_eq!(
            relational_count,
            LEGACY_RELATIONAL.len() - 4,
            "关系型规则应恰剪除 4 条冗余"
        );
        // "; EXECUTE" 场景仍由 "; EXEC" 命中
        assert!(engine_contains("SELECT 1; EXECUTE SP_X"));
        // "#" 场景仍命中（"ORDER BY 1#" 冗余副本剪除后行为不变）
        assert!(engine_contains("SELECT 1 ORDER BY 1#"));
    }

    /// 命中结果携带稳定规则 ID 与类别（供审计消费）
    #[test]
    fn test_scan_reports_rule_ids_and_categories() {
        let engine = InjectionEngine::global();
        let findings = engine.scan("SELECT 1; DROP TABLE T", &[RuleCategory::Stacked]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "stack.drop");
        assert_eq!(findings[0].category, RuleCategory::Stacked);
    }

    /// DDL 管线与图管线的独立可用性
    #[test]
    fn test_ddl_and_graph_pipelines() {
        let engine = InjectionEngine::global();
        let ddl = engine.scan_ddl("  drop database production  ");
        assert_eq!(ddl.len(), 1);
        assert_eq!(ddl[0].id, "ddl.drop_database");

        let graph = engine.scan_graph("MATCH (n) CALL dbms.components() YIELD * RETURN n");
        assert!(graph.iter().any(|r| r.id == "graph.call_dbms"));

        let comment = engine.scan_graph("MATCH (n) RETURN n /* sneaky */");
        assert!(comment.iter().any(|r| r.id == "comment.block_marker"));
    }

    #[cfg(feature = "sql-parser")]
    #[test]
    fn test_dynamic_variables_via_engine() {
        let engine = InjectionEngine::global();
        assert!(engine.has_dynamic_variables("SELECT * FROM t WHERE a = @var"));
        assert!(engine.has_dynamic_variables("SELECT ${col} FROM t"));
        assert!(!engine.has_dynamic_variables("SELECT * FROM t WHERE a = ?"));
    }
}
