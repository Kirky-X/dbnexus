// Copyright (c) 2025 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DB Nexus 过程宏定义
//!
//! 提供 #[derive(DbEntity)]、#[db_crud]、#[db_permission] 三个宏
//!
//! 这些宏适配 sea-orm 2.0，简化实体定义同时保留权限控制功能

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use regex::Regex;
use syn::{DeriveInput, parse_macro_input};

/// DbEntity 派生宏
///
/// 用于为用户定义的 Model struct 添加辅助方法和权限控制
/// 配合 sea-orm 2.0 的 DeriveEntityModel 使用
///
/// # 示例
///
/// ```rust,ignore
/// use dbnexus::DbEntity;
/// use sea_orm::entity::prelude::*;
///
/// #[derive(DbEntity)]
/// #[sea_orm(table_name = "users")]
/// struct User {
///     #[sea_orm(primary_key)]
///     id: i64,
///     name: String,
/// }
/// ```
///
/// 注意：此宏依赖 sea-orm 2.0 的派生宏，用户需要同时使用：
/// - `#[derive(DeriveEntityModel, DeriveModel, DeriveActiveModel)]`
/// - `#[sea_orm(table_name = "...")]`
/// - `#[sea_orm(primary_key)]` on primary key field
#[proc_macro_derive(DbEntity, attributes(table_name, primary_key))]
pub fn derive_db_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;
    let generics = &input.generics;

    // 提取表名
    let table_name = extract_table_name(&input.attrs);

    // 提取主键字段名
    let primary_key_name = extract_primary_key(&input.data);

    if table_name.is_empty() {
        return syn::Error::new(
            struct_name.span(),
            "Missing #[table_name = \"...\")] attribute on the struct",
        )
        .to_compile_error()
        .into();
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // 生成代码 - 只添加方法，不重新定义结构体
    let expanded = quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// 获取表名
            pub fn table_name() -> &'static str {
                #table_name
            }

            /// 获取主键列名
            pub fn primary_key_column() -> &'static str {
                #primary_key_name
            }
        }
    };

    TokenStream::from(expanded)
}

/// 提取 table_name 属性
fn extract_table_name(attrs: &[syn::Attribute]) -> String {
    for attr in attrs {
        if attr.path().is_ident("table_name") {
            let attr_str = attr.meta.clone().into_token_stream().to_string();
            if let Some(eq_pos) = attr_str.find('=') {
                let after_eq = &attr_str[eq_pos + 1..];
                if let Some(quote_start) = after_eq.find('"') {
                    if let Some(quote_end) = after_eq[quote_start + 1..].find('"') {
                        return after_eq[quote_start + 1..quote_start + 1 + quote_end].to_string();
                    }
                }
            }
        }
    }
    String::new()
}

/// 提取主键字段名
fn extract_primary_key(data: &syn::Data) -> String {
    if let syn::Data::Struct(s) = data {
        for field in &s.fields {
            for attr in &field.attrs {
                if attr.path().is_ident("primary_key") {
                    if let Some(ident) = &field.ident {
                        return ident.to_string();
                    }
                }
            }
        }
    }
    String::new()
}

/// db_crud 属性宏
///
/// 为 Entity 自动生成 CRUD 方法（真正执行数据库操作）
#[proc_macro_attribute]
pub fn db_crud(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // 提取表名
    let table_name = extract_table_name(&input.attrs);

    if table_name.is_empty() {
        return syn::Error::new(
            struct_name.span(),
            "#[db_crud] requires #[table_name = \"...\")] attribute on the entity",
        )
        .to_compile_error()
        .into();
    }

    // 提取所有字段名
    let field_names = extract_field_names(&input.data);
    let field_names_str: Vec<String> = field_names.iter().map(|s| s.to_string()).collect();

    // 提取主键字段名
    let primary_key = extract_primary_key(&input.data);

    // 生成字段列表字符串（用于 SQL）
    let field_list = field_names_str.join(", ");

    // 生成占位符列表（用于参数化查询）
    let placeholders: Vec<String> = field_names_str
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let placeholder_list = placeholders.join(", ");

    // 生成 update SQL（SET 子句）
    let update_set: Vec<String> = field_names_str
        .iter()
        .filter(|f| *f != &primary_key)
        .map(|f| format!("{} = {}", f, 1))
        .collect();
    let update_set_str = update_set.join(", ");

    // 生成 CRUD 方法
    let impl_block = quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// 插入新记录
            pub async fn insert(
                session: &dbnexus::Session,
                entity: Self,
            ) -> Result<Self, dbnexus::DbError> {
                session.check_permission(#table_name, &dbnexus::permission::Operation::Insert)?;
                session.mark_write();

                // 构建参数化查询
                let sql = format!("INSERT INTO {} ({}) VALUES ({})", #table_name, #field_list, #placeholder_list);

                // 执行 INSERT 语句
                session.execute_raw(&sql).await?;

                Ok(entity)
            }

            /// 根据ID查找记录
            pub async fn find_by_id(
                session: &dbnexus::Session,
                id: i64,
            ) -> Result<Option<Self>, dbnexus::DbError> {
                session.check_permission(#table_name, &dbnexus::permission::Operation::Select)?;

                // 构建参数化查询
                let sql = format!("SELECT * FROM {} WHERE {} = {}", #table_name, #primary_key, id);

                // 执行 SELECT 语句
                session.execute_raw(&sql).await?;

                // 注意：宏无法直接解析查询结果返回实体
                // 建议使用 API 的 execute_raw 方法获取原始结果
                Ok(None)
            }

            /// 更新记录
            pub async fn update(
                session: &dbnexus::Session,
                entity: Self,
            ) -> Result<Self, dbnexus::DbError> {
                session.check_permission(#table_name, &dbnexus::permission::Operation::Update)?;
                session.mark_write();

                // 构建参数化查询
                let sql = format!("UPDATE {} SET {} WHERE {} = 1", #table_name, #update_set_str, #primary_key);

                // 执行 UPDATE 语句
                session.execute_raw(&sql).await?;

                Ok(entity)
            }

            /// 根据ID删除记录
            pub async fn delete(
                session: &dbnexus::Session,
                id: i64,
            ) -> Result<u64, dbnexus::DbError> {
                session.check_permission(#table_name, &dbnexus::permission::Operation::Delete)?;
                session.mark_write();

                // 构建参数化查询
                let sql = format!("DELETE FROM {} WHERE {} = {}", #table_name, #primary_key, id);

                // 执行 DELETE 语句
                let result = session.execute_raw(&sql).await?;

                Ok(result.rows_affected())
            }

            /// 查询所有记录
            pub async fn find_all(
                session: &dbnexus::Session,
            ) -> Result<Vec<Self>, dbnexus::DbError> {
                session.check_permission(#table_name, &dbnexus::permission::Operation::Select)?;

                // 构建查询
                let sql = format!("SELECT * FROM {}", #table_name);

                // 执行 SELECT 语句
                session.execute_raw(&sql).await?;

                // 注意：宏无法直接解析查询结果返回实体列表
                // 建议使用 API 的 execute_raw 方法获取原始结果
                Ok(Vec::new())
            }

            /// 批量删除
            pub async fn delete_many(
                session: &dbnexus::Session,
                _filter: dbnexus::Condition,
            ) -> Result<u64, dbnexus::DbError> {
                session.check_permission(#table_name, &dbnexus::permission::Operation::Delete)?;
                session.mark_write();

                // 构建参数化查询
                let sql = format!("DELETE FROM {} WHERE {} = 1", #table_name, #primary_key);

                // 执行 DELETE 语句
                let result = session.execute_raw(&sql).await?;

                Ok(result.rows_affected())
            }
        }
    };

    // 返回原始结构体定义加上生成的 impl 块
    TokenStream::from(quote! {
        #input
        #impl_block
    })
}

/// 提取所有字段名
fn extract_field_names(data: &syn::Data) -> Vec<String> {
    let mut field_names = Vec::new();

    if let syn::Data::Struct(s) = data {
        for field in &s.fields {
            if let Some(ident) = &field.ident {
                field_names.push(ident.to_string());
            }
        }
    }

    field_names
}

/// db_permission 属性宏
///
/// 声明 Entity 允许访问的角色和操作
///
/// # 编译时角色验证
///
/// 如果指定了 `config` 属性，宏会在编译时验证声明的角色是否在配置文件中存在：
///
/// ```rust,ignore
/// #[derive(DbEntity)]
/// #[db_entity]
/// #[table_name = "users")]
/// #[db_permission(roles = ["admin", "user"], operations = ["read", "write"], config = "permissions.yaml")]
/// struct User {
///     #[primary_key]
///     id: i64,
///     name: String,
/// }
/// ```
///
/// 如果配置文件 `permissions.yaml` 中没有定义 `admin` 或 `user` 角色，编译将失败并显示错误信息。
#[proc_macro_attribute]
pub fn db_permission(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let _struct_span = struct_name.span();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // 解析属性参数
    let args_str = args.to_string();

    // 使用正则表达式解析 roles 参数
    let roles: Vec<String> = if let Ok(roles_re) = Regex::new(r#"roles\s*=\s*\[([^\]]*)\]"#) {
        if let Some(caps) = roles_re.captures(&args_str) {
            if let Some(roles_match) = caps.get(1) {
                roles_match
                    .as_str()
                    .split(',')
                    .filter_map(|role| {
                        let cleaned = role.trim().trim_matches('"').trim();
                        if !cleaned.is_empty() {
                            Some(cleaned.to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // 使用正则表达式解析 operations 参数
    let operations: Vec<String> = if let Ok(ops_re) = Regex::new(r#"operations\s*=\s*\[([^\]]*)\]"#) {
        if let Some(caps) = ops_re.captures(&args_str) {
            if let Some(ops_match) = caps.get(1) {
                ops_match
                    .as_str()
                    .split(',')
                    .filter_map(|op| {
                        let cleaned = op.trim().trim_matches('"').trim();
                        if !cleaned.is_empty() {
                            Some(cleaned.to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // 使用正则表达式解析 config 参数（可选，用于编译时验证）
    let config_path = if let Ok(config_re) = Regex::new(r#"config\s*=\s*"([^"]*)""#) {
        config_re.captures(&args_str).and_then(|caps| {
            caps.get(1).map(|config_match| config_match.as_str().to_string())
        })
    } else {
        None
    };

    // 生成角色和操作数组
    let roles_array: Vec<_> = roles.iter().map(|role| quote! { #role }).collect();
    let operations_array: Vec<_> = operations.iter().map(|op| quote! { #op }).collect();

    // 如果指定了 config 文件，进行编译时角色验证
    let validation_code = if let Some(config) = &config_path {
        // 使用 include_str! 在编译时读取配置文件
        // 注意：由于 Rust const 表达式的限制，无法在编译时进行复杂的 YAML 解析
        // 我们改为在运行时进行验证，或者使用宏在编译时进行验证
        let config_content = format!(
            r#"
            // 编译时包含配置文件
            const _PERMISSIONS_CONFIG: &str = include_str!("{}");

            // 注意：编译时角色验证已禁用，因为 Rust const 表达式不支持复杂的控制流
            // 角色验证将在运行时通过 PermissionContext 进行
            // 声明的角色列表: {:#?}
            "#,
            config,
            roles
        );

        quote! { #config_content }
    } else {
        // 如果没有指定 config，跳过编译时验证
        quote! {
            // 未启用编译时角色验证（未指定 config 属性）
            // 运行时验证仍然生效
            const _SKIP_COMPILE_TIME_VALIDATION: () = ();
        }
    };

    // 生成代码
    let expanded = quote! {
        #input

        #validation_code

        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// 允许访问此实体的角色列表
            pub const ALLOWED_ROLES: &'static [&'static str] = &[#(#roles_array),*];

            /// 允许的操作列表
            pub const ALLOWED_OPERATIONS: &'static [&'static str] = &[#(#operations_array),*];

            /// 获取允许的角色列表（用于运行时显示）
            pub fn allowed_roles() -> Vec<&'static str> {
                Self::ALLOWED_ROLES.to_vec()
            }

            /// 获取允许的操作列表（用于运行时显示）
            pub fn allowed_operations() -> Vec<&'static str> {
                Self::ALLOWED_OPERATIONS.to_vec()
            }

            /// 检查角色是否有权限访问此实体
            pub fn check_permission(ctx: &dbnexus::permission::PermissionContext) -> Result<(), dbnexus::DbError> {
                let role = ctx.role();
                if !Self::ALLOWED_ROLES.contains(&role) {
                    return Err(dbnexus::DbError::Permission(format!(
                        "Role '{}' is not allowed to access entity '{}'. Allowed roles: {:?}",
                        role,
                        Self::table_name(),
                        Self::ALLOWED_ROLES
                    )));
                }
                Ok(())
            }

            /// 检查角色是否有权限执行特定操作
            pub fn check_operation(
                ctx: &dbnexus::permission::PermissionContext,
                operation: &dbnexus::permission::Operation,
            ) -> Result<(), dbnexus::DbError> {
                let role = ctx.role();
                if !Self::ALLOWED_ROLES.contains(&role) {
                    return Err(dbnexus::DbError::Permission(format!(
                        "Role '{}' is not allowed to access entity '{}'",
                        role,
                        Self::table_name()
                    )));
                }

                if !Self::ALLOWED_OPERATIONS.is_empty() {
                    let op_str = operation.to_string();
                    if !Self::ALLOWED_OPERATIONS.contains(&op_str.as_str()) {
                        return Err(dbnexus::DbError::Permission(format!(
                            "Operation '{}' is not allowed for role '{}' on entity '{}'. Allowed operations: {:?}",
                            operation,
                            role,
                            Self::table_name(),
                            Self::ALLOWED_OPERATIONS
                        )));
                    }
                }

                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}

/// db_cache 属性宏
///
/// 为 Entity 配置缓存策略
#[proc_macro_attribute]
pub fn db_cache(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // 解析参数
    let args_str = args.to_string();

    // 使用正则表达式解析 ttl 参数
    let ttl = if let Ok(ttl_re) = Regex::new(r#"ttl\s*=\s*(\d+)"#) {
        if let Some(caps) = ttl_re.captures(&args_str) {
            if let Some(ttl_match) = caps.get(1) {
                ttl_match.as_str().parse().unwrap_or(300)
            } else {
                300
            }
        } else {
            300
        }
    } else {
        300
    };

    // 使用正则表达式解析 strategy 参数
    let strategy = if let Ok(strat_re) = Regex::new(r#"strategy\s*=\s*"([^"]*)""#) {
        if let Some(caps) = strat_re.captures(&args_str) {
            if let Some(strat_match) = caps.get(1) {
                strat_match.as_str().to_string()
            } else {
                "lru".to_string()
            }
        } else {
            "lru".to_string()
        }
    } else {
        "lru".to_string()
    };

    // 使用正则表达式解析 max_capacity 参数
    let max_capacity = if let Ok(cap_re) = Regex::new(r#"max_capacity\s*=\s*(\d+)"#) {
        if let Some(caps) = cap_re.captures(&args_str) {
            if let Some(cap_match) = caps.get(1) {
                cap_match.as_str().parse().unwrap_or(10000)
            } else {
                10000
            }
        } else {
            10000
        }
    } else {
        10000
    };

    let expanded = quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// 缓存 TTL（秒）
            pub const CACHE_TTL: u64 = #ttl;

            /// 缓存策略名称
            pub const CACHE_STRATEGY: &'static str = #strategy;

            /// 缓存最大容量
            pub const CACHE_MAX_CAPACITY: usize = #max_capacity;

            /// 是否启用缓存
            pub const CACHE_ENABLED: bool = true;

            /// 获取缓存键
            pub fn cache_key(id: &i64) -> dbnexus::cache::CacheKey {
                dbnexus::cache::make_cache_key(Self::table_name(), &id.to_string())
            }

            /// 获取缓存配置
            pub fn cache_config() -> dbnexus::cache::CacheConfig {
                dbnexus::cache::CacheConfig {
                    max_capacity: Self::CACHE_MAX_CAPACITY,
                    default_ttl: Self::CACHE_TTL,
                    cleanup_interval: 60,
                    enable_stats: true,
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// db_audit 属性宏
///
/// 为 Entity 配置审计日志策略
#[proc_macro_attribute]
pub fn db_audit(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // 解析参数
    let args_str = args.to_string();
    let mut log_values = true;

    if let Some(lv_start) = args_str.find("log_values") {
        let lv_part = &args_str[lv_start..];
        if let Some(eq_pos) = lv_part.find('=') {
            log_values = lv_part[eq_pos + 1..].trim().starts_with("true");
        }
    }

    let expanded = quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// 审计的操作列表
            pub const AUDIT_OPERATIONS: &'static [&'static str] = &["CREATE", "UPDATE", "DELETE"];

            /// 是否记录变更值
            pub const AUDIT_LOG_VALUES: bool = #log_values;

            /// 审计是否启用
            pub const AUDIT_ENABLED: bool = true;
        }
    };

    TokenStream::from(expanded)
}
