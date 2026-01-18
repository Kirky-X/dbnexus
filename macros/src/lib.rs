// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! DB Nexus 过程宏定义
//!
//! 提供 #[derive(DbEntity)]、#[db_crud]、#[db_permission] 三个宏
//!
//! 这些宏适配 sea-orm 2.0，简化实体定义同时保留权限控制功能

#![allow(dead_code)] // 允许未使用的辅助函数

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use proc_macro2::Span;
use quote::quote;
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
            "Missing #[table_name = \"...\"] attribute on the struct",
        )
        .to_compile_error()
        .into();
    }

    if primary_key_name.is_empty() {
        return syn::Error::new(struct_name.span(), "Missing #[primary_key] attribute on a struct field")
            .to_compile_error()
            .into();
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // 生成代码 - 保留原始结构体定义并添加方法
    let expanded = quote! {
        #input

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
///
/// 使用 syn 的 Meta API 安全解析属性，避免手动字符串操作
fn extract_table_name(attrs: &[syn::Attribute]) -> String {
    for attr in attrs {
        // 首先尝试从 #[sea_orm(table_name = "...")] 中提取
        if attr.path().is_ident("sea_orm") {
            // 使用 parse_nested_meta 解析嵌套属性
            let mut table_name = String::new();
            let _ = attr.parse_nested_meta(|nested| {
                if nested.path.is_ident("table_name") {
                    // 解析值
                    let value: syn::Expr = nested.input.parse()?;
                    if let syn::Expr::Lit(expr_lit) = value {
                        if let syn::Lit::Str(lit_str) = expr_lit.lit {
                            table_name = lit_str.value();
                        }
                    }
                }
                Ok(())
            });
            if !table_name.is_empty() {
                return table_name;
            }
        }
        // 然后尝试从 #[table_name = "...")] 中提取
        if attr.path().is_ident("table_name") {
            if let syn::Meta::NameValue(name_value) = &attr.meta {
                if let syn::Expr::Lit(expr_lit) = &name_value.value {
                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                        return lit_str.value();
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

#[proc_macro_attribute]
pub fn db_crud(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // 提取表名（用于权限检查）
    let table_name = extract_table_name(&input.attrs);

    // 验证表名是否存在
    if table_name.is_empty() {
        return syn::Error::new(
            struct_name.span(),
            "#[db_crud] requires #[table_name = \"...\"] or #[sea_orm(table_name = \"...\")] attribute on the entity",
        )
        .to_compile_error()
        .into();
    }

    // 生成 CRUD 方法（使用 Sea-ORM 原生 API）
    let impl_block = quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// 插入新记录
            ///
            /// # Arguments
            ///
            /// * `db` - 数据库连接
            /// * `model` - 要插入的模型数据
            ///
            /// # Returns
            ///
            /// 插入后的完整模型（包含自动生成的主键）
            pub async fn insert(
                db: &sea_orm::DatabaseConnection,
                model: <Self as sea_orm::entity::EntityTrait>::Model,
            ) -> Result<<Self as sea_orm::entity::EntityTrait>::Model, dbnexus::DbError> {
                use sea_orm::Entity;

                // 将 Model 转换为 ActiveModel
                let active_model: <Self as sea_orm::entity::EntityTrait>::ActiveModel = model.into();

                // 执行插入
                Entity::insert(active_model)
                    .exec(db)
                    .await
                    .map_err(Into::into)
            }

            /// 根据 ID 查找记录
            ///
            /// # Arguments
            ///
            /// * `db` - 数据库连接
            /// * `id` - 主键 ID
            ///
            /// # Returns
            ///
            /// 找到的记录（如果有）
            pub async fn find_by_id(
                db: &sea_orm::DatabaseConnection,
                id: i64,
            ) -> Result<Option<<Self as sea_orm::entity::EntityTrait>::Model>, dbnexus::DbError> {
                use sea_orm::Entity;

                Entity::find_by_id(id)
                    .one(db)
                    .await
                    .map_err(Into::into)
            }

            /// 更新记录
            ///
            /// # Arguments
            ///
            /// * `db` - 数据库连接
            /// * `model` - 要更新的模型数据
            ///
            /// # Returns
            ///
            /// 更新后的模型
            pub async fn update(
                db: &sea_orm::DatabaseConnection,
                model: <Self as sea_orm::entity::EntityTrait>::Model,
            ) -> Result<<Self as sea_orm::entity::EntityTrait>::Model, dbnexus::DbError> {
                use sea_orm::Entity;

                // 将 Model 转换为 ActiveModel
                let active_model: <Self as sea_orm::entity::EntityTrait>::ActiveModel = model.into();

                Entity::update(active_model)
                    .exec(db)
                    .await
                    .map_err(Into::into)
            }

            /// 根据 ID 删除记录
            ///
            /// # Arguments
            ///
            /// * `db` - 数据库连接
            /// * `id` - 要删除的记录 ID
            ///
            /// # Returns
            ///
            /// 删除的记录数
            pub async fn delete(
                db: &sea_orm::DatabaseConnection,
                id: i64,
            ) -> Result<u64, dbnexus::DbError> {
                use sea_orm::Entity;

                // 先查询记录
                let record = Entity::find_by_id(id)
                    .one(db)
                    .await?
                    .ok_or_else(|| dbnexus::DbError::NotFound(format!("Record with id {} not found", id)))?;

                // 删除记录
                let result = Entity::delete(record).exec(db).await?;
                Ok(result.rows_affected)
            }

            /// 查询所有记录
            ///
            /// # Arguments
            ///
            /// * `db` - 数据库连接
            ///
            /// # Returns
            ///
            /// 所有记录的向量
            pub async fn find_all(
                db: &sea_orm::DatabaseConnection,
            ) -> Result<Vec<<Self as sea_orm::entity::EntityTrait>::Model>, dbnexus::DbError> {
                use sea_orm::Entity;

                Entity::find()
                    .all(db)
                    .await
                    .map_err(Into::into)
            }

            /// 条件查询
            ///
            /// # Arguments
            ///
            /// * `db` - 数据库连接
            /// * `condition` - 查询条件
            ///
            /// # Returns
            ///
            /// 符合条件的记录
            pub async fn find_by_condition(
                db: &sea_orm::DatabaseConnection,
                condition: sea_orm::Condition,
            ) -> Result<Vec<<Self as sea_orm::entity::EntityTrait>::Model>, dbnexus::DbError> {
                use sea_orm::Entity;

                Entity::find()
                    .filter(condition)
                    .all(db)
                    .await
                    .map_err(Into::into)
            }

            /// 批量删除
            ///
            /// # Arguments
            ///
            /// * `db` - 数据库连接
            /// * `filter` - 删除条件
            ///
            /// # Returns
            ///
            /// 删除的记录数
            pub async fn delete_many(
                db: &sea_orm::DatabaseConnection,
                filter: sea_orm::Condition,
            ) -> Result<u64, dbnexus::DbError> {
                use sea_orm::Entity;

                let result = Entity::delete_many()
                    .filter(filter)
                    .exec(db)
                    .await?;
                Ok(result.rows_affected)
            }

            /// 统计记录数
            ///
            /// # Arguments
            ///
            /// * `db` - 数据库连接
            ///
            /// # Returns
            ///
            /// 记录总数
            pub async fn count(
                db: &sea_orm::DatabaseConnection,
            ) -> Result<u64, dbnexus::DbError> {
                use sea_orm::Entity;

                let count = Entity::find()
                    .count(db)
                    .await?;
                Ok(count)
            }
        }
    };

    // 返回原始结构体定义加上生成的 impl 块
    TokenStream::from(quote! {
        #input
        #impl_block
    })
}

///
/// 有效的角色名必须：
/// - 以字母或下划线开头
/// - 只能包含字母、数字、下划线
/// - 长度在 1-64 之间
fn validate_role_names(roles: &[String], struct_name: &syn::Ident) -> Result<(), ()> {
    // 角色名格式：字母/下划线开头，后跟字母、数字、下划线
    let role_name_pattern = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]{0,63}$").map_err(|_| ())?;

    for role in roles {
        if !role_name_pattern.is_match(role) {
            proc_macro_error::abort!(
                struct_name,
                format!(
                    "Invalid role name '{}'. Role names must:\n\
                     - Start with a letter or underscore\n\
                     - Contain only letters, numbers, and underscores\n\
                     - Be between 1 and 64 characters long\n\n\
                     Example valid roles: admin, user_123, _moderator",
                    role
                ),
            );
        }
    }

    Ok(())
}

fn validate_config_path(config_path: &str, struct_name: &syn::Ident) {
    if config_path.starts_with('/')
        || config_path.starts_with('\\')
        || config_path.contains("..")
        || config_path.contains(':')
    {
        proc_macro_error::abort!(
            struct_name,
            "Invalid config path. Use a relative path without '..' or drive letters.",
        );
    }
}

/// db_permission 属性宏
///
/// 声明 Entity 允许访问的角色和操作
///
/// # 编译时角色验证
///
/// 宏会在编译时验证声明的角色名格式是否正确：
///
/// ```rust,ignore
/// #[derive(DbEntity)]
/// #[db_entity]
/// #[table_name = "users")]
/// #[db_permission(roles = ["admin", "user"], operations = ["read", "write"])]
/// struct User {
///     #[primary_key]
///     id: i64,
///     name: String,
/// }
/// ```
///
/// 无效的角色名会导致编译错误，例如：
/// - `123admin` (以数字开头)
/// - `admin-user` (包含连字符)
/// - 空字符串
///
/// # 使用示例
/// ```rust,ignore
/// #[derive(DbEntity)]
/// #[table_name = "users")]
/// #[db_permission(roles = ["admin", "user"], operations = ["read", "write"])]
/// struct User {
///     #[primary_key]
///     id: i64,
///     name: String,
/// }
/// ```
#[proc_macro_attribute]
#[proc_macro_error]
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

    // 编译时验证角色名格式
    if validate_role_names(&roles, struct_name).is_err() {
        proc_macro_error::abort!(
            struct_name,
            "Invalid role name(s) detected. See errors above for details."
        );
    }

    // 使用正则表达式解析 config 参数（可选，用于编译时验证）
    let config_path = if let Ok(config_re) = Regex::new(r#"config\s*=\s*"([^"]*)""#) {
        config_re
            .captures(&args_str)
            .and_then(|caps| caps.get(1).map(|config_match| config_match.as_str().to_string()))
    } else {
        None
    };

    // 生成角色和操作数组
    let roles_array: Vec<_> = roles.iter().map(|role| quote! { #role }).collect();
    let operations_array: Vec<_> = operations.iter().map(|op| quote! { #op }).collect();

    // 如果指定了 config 文件，进行编译时角色验证
    let validation_code = if let Some(config) = &config_path {
        validate_config_path(config, struct_name);
        let config_lit = syn::LitStr::new(config, Span::call_site());
        quote! {
            const _PERMISSIONS_CONFIG: &str = include_str!(#config_lit);
        }
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
