// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! DB Nexus 过程宏定义
//!
//! 提供 `#[db_entity]` 统一属性宏，替代旧版的 `DbEntity`（derive）、`db_crud`、`db_permission`、
//! `db_cache`、`db_audit` 五个分散宏。
//!
//! 这些宏适配 sea-orm 2.0，简化实体定义同时保留权限控制功能。

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use regex::Regex;
use syn::{DeriveInput, parse::Parser, parse_macro_input, spanned::Spanned};

// ============================================================================
// db_entity 统一属性宏（替代 DbEntity + db_crud + db_permission + db_cache + db_audit）
// ============================================================================

/// `db_entity` 属性宏参数
///
/// 解析 `#[db_entity(table_name = "...", primary_key = "...", timestamps = true, ...)]` 参数
#[derive(Default)]
struct DbEntityArgs {
    /// 表名（必需）
    table_name: String,
    /// 主键字段名（必需，修复 db_crud 硬编码 `id` 的 bug）
    primary_key: String,
    /// 启用自动时间戳（可选）
    timestamps: bool,
    /// 启用软删除（可选）
    soft_delete: bool,
    /// 启用数据验证（可选）
    validate: bool,
    /// hooks 嵌套参数解析结果（可选）
    hooks: HooksArgs,
    /// permissions 参数已解析（可选）
    has_permissions: bool,
    /// cache 参数已解析（可选）
    has_cache: bool,
    /// audit 参数已解析（可选）
    has_audit: bool,
    /// permissions 嵌套参数原始 token（Phase 3 实现解析）
    permissions_tokens: Option<proc_macro2::TokenStream>,
    /// cache 嵌套参数原始 token（Phase 3 实现解析）
    cache_tokens: Option<proc_macro2::TokenStream>,
    /// audit 嵌套参数原始 token（Phase 3 实现解析）
    audit_tokens: Option<proc_macro2::TokenStream>,
}

/// `hooks(...)` 嵌套参数解析结果
///
/// 解析 `hooks(before_insert = "fn_name", after_insert = "fn_name", ...)` 参数
/// 每个 hook 函数名以字符串字面量指定，宏生成代码时转为 ident 调用
#[derive(Default)]
struct HooksArgs {
    /// `before_insert = "fn_name"` — insert 前钩子，签名 `fn(&mut ActiveModel) -> Result<(), E>`
    before_insert: Option<String>,
    /// `after_insert = "fn_name"` — insert 后钩子，签名 `fn(&Model) -> Result<(), E>`
    after_insert: Option<String>,
    /// `before_update = "fn_name"` — update 前钩子，签名 `fn(&mut ActiveModel) -> Result<(), E>`
    before_update: Option<String>,
    /// `after_update = "fn_name"` — update 后钩子，签名 `fn(&Model) -> Result<(), E>`
    after_update: Option<String>,
    /// `before_delete = "fn_name"` — delete 前钩子，签名 `fn(&mut ActiveModel) -> Result<(), E>`
    before_delete: Option<String>,
    /// `after_delete = "fn_name"` — delete 后钩子，签名 `fn(&Model) -> Result<(), E>`
    after_delete: Option<String>,
}

impl HooksArgs {
    /// 是否有 before_save 相关 hook（before_insert 或 before_update）
    fn has_before_save_hooks(&self) -> bool {
        self.before_insert.is_some() || self.before_update.is_some()
    }

    /// 是否有 after_save 相关 hook（after_insert 或 after_update）
    fn has_after_save_hooks(&self) -> bool {
        self.after_insert.is_some() || self.after_update.is_some()
    }
}

/// 解析 `hooks(...)` 嵌套参数
///
/// 支持的参数格式（所有参数可选，字符串字面量）：
/// - `before_insert = "fn_name"`
/// - `after_insert = "fn_name"`
/// - `before_update = "fn_name"`
/// - `after_update = "fn_name"`
/// - `before_delete = "fn_name"`
/// - `after_delete = "fn_name"`
fn parse_hooks_args(tokens: proc_macro2::TokenStream) -> Result<HooksArgs, syn::Error> {
    let mut result = HooksArgs::default();

    let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
    let parsed = syn::parse::Parser::parse(&parser, tokens.into())?;

    for meta in parsed {
        if let syn::Meta::NameValue(nv) = meta {
            let key = nv.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s), ..
            }) = &nv.value
            {
                let value = s.value();
                match key.as_str() {
                    "before_insert" => result.before_insert = Some(value),
                    "after_insert" => result.after_insert = Some(value),
                    "before_update" => result.before_update = Some(value),
                    "after_update" => result.after_update = Some(value),
                    "before_delete" => result.before_delete = Some(value),
                    "after_delete" => result.after_delete = Some(value),
                    _ => {
                        return Err(syn::Error::new(
                            nv.path.span(),
                            format!(
                                "unknown hook parameter '{}', expected one of: \
                                 before_insert, after_insert, before_update, after_update, \
                                 before_delete, after_delete",
                                key
                            ),
                        ));
                    }
                }
            } else {
                return Err(syn::Error::new(
                    nv.value.span(),
                    format!("hook parameter '{}' must be a string literal", key),
                ));
            }
        } else {
            return Err(syn::Error::new(
                meta.span(),
                "hooks() parameters must be in `key = \"value\"` form",
            ));
        }
    }

    Ok(result)
}

/// 解析 `db_entity` 宏参数
///
/// 支持的参数格式：
/// - `table_name = "users"` （必需，字符串字面量）
/// - `primary_key = "id"` （必需，字符串字面量）
/// - `timestamps = true` （可选，布尔字面量）
/// - `soft_delete = true` （可选，布尔字面量）
/// - `validate` （可选，布尔开关，无值）
/// - `hooks(...)` （可选，嵌套参数，Phase 3 实现）
/// - `permissions(...)` （可选，嵌套参数）
/// - `cache(...)` （可选，嵌套参数）
/// - `audit(...)` （可选，嵌套参数）
fn parse_db_entity_args(args: TokenStream) -> Result<DbEntityArgs, syn::Error> {
    let mut result = DbEntityArgs::default();

    let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
    let parsed = syn::parse::Parser::parse(&parser, args)?;

    for meta in parsed {
        match &meta {
            // table_name = "..."
            syn::Meta::NameValue(nv) if nv.path.is_ident("table_name") => {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s), ..
                }) = &nv.value
                {
                    result.table_name = s.value();
                } else {
                    return Err(syn::Error::new(nv.value.span(), "table_name must be a string literal"));
                }
            }
            // primary_key = "..."
            syn::Meta::NameValue(nv) if nv.path.is_ident("primary_key") => {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s), ..
                }) = &nv.value
                {
                    result.primary_key = s.value();
                } else {
                    return Err(syn::Error::new(nv.value.span(), "primary_key must be a string literal"));
                }
            }
            // timestamps = true|false
            syn::Meta::NameValue(nv) if nv.path.is_ident("timestamps") => {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Bool(b), ..
                }) = &nv.value
                {
                    result.timestamps = b.value;
                } else {
                    return Err(syn::Error::new(
                        nv.value.span(),
                        "timestamps must be a boolean literal (true|false)",
                    ));
                }
            }
            // soft_delete = true|false
            syn::Meta::NameValue(nv) if nv.path.is_ident("soft_delete") => {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Bool(b), ..
                }) = &nv.value
                {
                    result.soft_delete = b.value;
                } else {
                    return Err(syn::Error::new(
                        nv.value.span(),
                        "soft_delete must be a boolean literal (true|false)",
                    ));
                }
            }
            // validate （布尔开关，无值）
            syn::Meta::Path(p) if p.is_ident("validate") => {
                result.validate = true;
            }
            // hooks(...) — 嵌套参数解析为 HooksArgs
            syn::Meta::List(list) if list.path.is_ident("hooks") => {
                result.hooks = parse_hooks_args(list.tokens.clone())?;
            }
            // permissions(...) — 嵌套参数
            syn::Meta::List(list) if list.path.is_ident("permissions") => {
                result.has_permissions = true;
                result.permissions_tokens = Some(list.tokens.clone());
            }
            // cache(...) — 嵌套参数
            syn::Meta::List(list) if list.path.is_ident("cache") => {
                result.has_cache = true;
                result.cache_tokens = Some(list.tokens.clone());
            }
            // audit(...) — 嵌套参数
            syn::Meta::List(list) if list.path.is_ident("audit") => {
                result.has_audit = true;
                result.audit_tokens = Some(list.tokens.clone());
            }
            _ => {
                let name = match &meta {
                    syn::Meta::Path(p) => p.get_ident().map(|i| i.to_string()).unwrap_or_default(),
                    syn::Meta::NameValue(nv) => nv.path.get_ident().map(|i| i.to_string()).unwrap_or_default(),
                    syn::Meta::List(l) => l.path.get_ident().map(|i| i.to_string()).unwrap_or_default(),
                };
                return Err(syn::Error::new(
                    meta.span(),
                    format!(
                        "unknown #[db_entity] parameter '{}', expected one of: \
                         table_name, primary_key, timestamps, soft_delete, validate, \
                         hooks, permissions, cache, audit",
                        name
                    ),
                ));
            }
        }
    }

    Ok(result)
}

// ============================================================================
// 嵌套参数解析辅助函数（permissions/cache/audit 子参数）
// ============================================================================

/// 解析嵌套的 name = value 参数对，返回 (参数名, 参数名 span, 值 token) 列表
fn parse_nested_params(
    tokens: &proc_macro2::TokenStream,
) -> Result<Vec<(String, proc_macro2::Span, proc_macro2::TokenStream)>, syn::Error> {
    let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
    let parsed = parser.parse2(tokens.clone())?;

    let mut result = Vec::new();
    for meta in parsed {
        match meta {
            syn::Meta::NameValue(nv) => {
                if let Some(ident) = nv.path.get_ident() {
                    result.push((ident.to_string(), nv.path.span(), nv.value.to_token_stream()));
                }
            }
            syn::Meta::Path(p) => {
                if let Some(ident) = p.get_ident() {
                    result.push((ident.to_string(), p.span(), proc_macro2::TokenStream::new()));
                }
            }
            syn::Meta::List(list) => {
                let name = list.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                return Err(syn::Error::new(
                    list.path.span(),
                    format!(
                        "unexpected nested list parameter '{}', expected name = value pair",
                        name
                    ),
                ));
            }
        }
    }
    Ok(result)
}

/// 从 token 中提取字符串数组（如 `["admin", "manager"]` → `vec!["admin", "manager"]`）
fn extract_string_array(tokens: &proc_macro2::TokenStream) -> Result<Vec<String>, syn::Error> {
    let expr: syn::Expr = syn::parse2(tokens.clone())?;
    match expr {
        syn::Expr::Array(arr) => {
            let mut strings = Vec::new();
            for elem in arr.elems {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s), ..
                }) = elem
                {
                    strings.push(s.value());
                } else {
                    return Err(syn::Error::new(elem.span(), "Expected string literal in array"));
                }
            }
            Ok(strings)
        }
        syn::Expr::Reference(r) => extract_string_array(&r.expr.to_token_stream()),
        _ => Err(syn::Error::new(expr.span(), "Expected array of strings")),
    }
}

/// 从 token 中提取字符串字面量
fn extract_string(tokens: &proc_macro2::TokenStream) -> Result<String, syn::Error> {
    let expr: syn::Expr = syn::parse2(tokens.clone())?;
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s), ..
        }) => Ok(s.value()),
        _ => Err(syn::Error::new(expr.span(), "Expected string literal")),
    }
}

/// 从 token 中提取 u64 整数字面量
fn extract_u64(tokens: &proc_macro2::TokenStream) -> Result<u64, syn::Error> {
    let expr: syn::Expr = syn::parse2(tokens.clone())?;
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(i), ..
        }) => Ok(i.base10_parse()?),
        _ => Err(syn::Error::new(expr.span(), "Expected integer literal")),
    }
}

/// 从 token 中提取布尔字面量
fn extract_bool(tokens: &proc_macro2::TokenStream) -> Result<bool, syn::Error> {
    let expr: syn::Expr = syn::parse2(tokens.clone())?;
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Bool(b), ..
        }) => Ok(b.value),
        _ => Err(syn::Error::new(expr.span(), "Expected boolean literal")),
    }
}

/// 解析 permissions(roles = [...], operations = [...]) 嵌套参数
struct PermissionsParams {
    roles: Vec<String>,
    operations: Vec<String>,
}

fn parse_permissions_params(
    tokens: &proc_macro2::TokenStream,
    struct_name: &syn::Ident,
) -> Result<PermissionsParams, syn::Error> {
    let params = parse_nested_params(tokens)?;
    let mut roles = Vec::new();
    let mut operations = Vec::new();

    for (name, span, value_tokens) in params {
        match name.as_str() {
            "roles" => {
                roles = extract_string_array(&value_tokens)?;
                validate_role_names(&roles, struct_name)
                    .map_err(|_| syn::Error::new(struct_name.span(), "Invalid role name format in permissions()"))?;
            }
            "operations" => {
                operations = extract_string_array(&value_tokens)?;
            }
            _ => {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "unknown parameter '{}' in permissions(), expected one of: roles, operations",
                        name
                    ),
                ));
            }
        }
    }

    Ok(PermissionsParams { roles, operations })
}

/// 解析 cache(ttl = 60, strategy = "lru", max_capacity = 5000) 嵌套参数
struct CacheParams {
    ttl: u64,
    strategy: String,
    max_capacity: usize,
}

fn parse_cache_params(tokens: &proc_macro2::TokenStream) -> Result<CacheParams, syn::Error> {
    let params = parse_nested_params(tokens)?;
    let mut ttl: u64 = 300;
    let mut strategy = String::from("lru");
    let mut max_capacity: usize = 10000;

    for (name, span, value_tokens) in params {
        match name.as_str() {
            "ttl" => ttl = extract_u64(&value_tokens)?,
            "strategy" => strategy = extract_string(&value_tokens)?,
            "max_capacity" => {
                let cap = extract_u64(&value_tokens)?;
                max_capacity = cap as usize;
            }
            _ => {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "unknown parameter '{}' in cache(), expected one of: ttl, strategy, max_capacity",
                        name
                    ),
                ));
            }
        }
    }

    Ok(CacheParams {
        ttl,
        strategy,
        max_capacity,
    })
}

/// 解析 audit(table_name = "...", log_values = true, operations = [...], roles = [...]) 嵌套参数
struct AuditParams {
    table_name: String,
    log_values: bool,
    operations: Vec<String>,
    roles: Vec<String>,
}

fn parse_audit_params(tokens: &proc_macro2::TokenStream, struct_name: &syn::Ident) -> Result<AuditParams, syn::Error> {
    let params = parse_nested_params(tokens)?;
    let mut table_name = String::new();
    let mut log_values = true;
    let mut operations = vec!["INSERT".to_string(), "UPDATE".to_string(), "DELETE".to_string()];
    let mut roles = vec!["admin".to_string()];

    for (name, span, value_tokens) in params {
        match name.as_str() {
            "table_name" => table_name = extract_string(&value_tokens)?,
            "log_values" => log_values = extract_bool(&value_tokens)?,
            "operations" => operations = extract_string_array(&value_tokens)?,
            "roles" => {
                roles = extract_string_array(&value_tokens)?;
                validate_role_names(&roles, struct_name)
                    .map_err(|_| syn::Error::new(struct_name.span(), "Invalid role name format in audit()"))?;
            }
            _ => {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "unknown parameter '{}' in audit(), expected one of: table_name, log_values, operations, roles",
                        name
                    ),
                ));
            }
        }
    }

    Ok(AuditParams {
        table_name,
        log_values,
        operations,
        roles,
    })
}

/// `db_entity` 统一属性宏
///
/// 替代 `DbEntity`（derive）+ `db_crud` + `db_permission` + `db_cache` + `db_audit` 五个分散宏。
/// 一次解析结构体及其属性，统一生成所有能力。
///
/// # 必需参数
///
/// - `table_name = "..."` — 表名
/// - `primary_key = "..."` — 主键字段名（修复 `db_crud` 硬编码 `id` 的 bug）
///
/// # 可选参数
///
/// - `timestamps = true` — 启用自动时间戳（Phase 3 实现）
/// - `soft_delete = true` — 启用软删除（Phase 3 实现）
/// - `validate` — 启用数据验证（Phase 3 实现）
/// - `hooks(...)` — 事件钩子（Phase 3 实现）
/// - `permissions(...)` — 权限控制
/// - `cache(...)` — 缓存配置
/// - `audit(...)` — 审计配置
///
/// # 示例
///
/// ```rust,ignore
/// use dbnexus::db_entity;
/// use sea_orm::entity::prelude::*;
///
/// #[db_entity(table_name = "users", primary_key = "id")]
/// #[derive(DeriveEntityModel, DeriveModel, DeriveActiveModel)]
/// pub struct Model {
///     #[sea_orm(primary_key)]
///     pub id: i64,
///     pub name: String,
/// }
/// ```
///
/// # 主键字段名非 `id` 的示例
///
/// ```rust,ignore
/// #[db_entity(table_name = "users", primary_key = "user_id")]
/// pub struct Model {
///     #[sea_orm(primary_key)]
///     pub user_id: i64,
///     pub name: String,
/// }
/// ```
///
/// 此时 `update` 方法会访问 `active_model.user_id` 而非 `active_model.id`。
#[proc_macro_attribute]
pub fn db_entity(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // 解析参数
    let entity_args = match parse_db_entity_args(args) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error().into(),
    };

    // 验证必需参数
    if entity_args.table_name.is_empty() {
        return syn::Error::new(
            struct_name.span(),
            "#[db_entity] requires `table_name = \"...\"` parameter",
        )
        .to_compile_error()
        .into();
    }
    if entity_args.primary_key.is_empty() {
        return syn::Error::new(
            struct_name.span(),
            "#[db_entity] requires `primary_key = \"...\"` parameter",
        )
        .to_compile_error()
        .into();
    }

    let table_name = &entity_args.table_name;
    let primary_key_str = &entity_args.primary_key;
    let primary_key_ident = syn::Ident::new(primary_key_str, proc_macro2::Span::call_site());
    // PascalCase 版本用于 Column 枚举变体（如 "id" → Column::Id）
    let primary_key_pascal = {
        let mut pascal = String::new();
        let mut capitalize_next = true;
        for ch in primary_key_str.chars() {
            if ch == '_' {
                capitalize_next = true;
            } else if capitalize_next {
                pascal.extend(ch.to_uppercase());
                capitalize_next = false;
            } else {
                pascal.push(ch);
            }
        }
        syn::Ident::new(&pascal, proc_macro2::Span::call_site())
    };

    // Task 6.5: soft_delete = true 时自动注入 deleted_at 字段（若用户未定义）
    //
    // 检查 struct 是否已有 deleted_at 字段，如果没有且 soft_delete=true，则自动添加。
    // 修改 input 会影响 #input 的输出，确保 DeriveEntityModel 能看到 deleted_at 字段。
    let has_deleted_at_field = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .any(|f| f.ident.as_ref().map(|i| i == "deleted_at").unwrap_or(false)),
            _ => false,
        },
        _ => false,
    };

    if entity_args.soft_delete && !has_deleted_at_field {
        if let syn::Data::Struct(ref mut data) = input.data {
            if let syn::Fields::Named(ref mut fields) = data.fields {
                let deleted_at_field: syn::Field = syn::parse_quote! {
                    pub deleted_at: Option<time::OffsetDateTime>
                };
                fields.named.push(deleted_at_field);
            }
        }
    }

    // 解析 permissions/cache/audit 嵌套参数
    let permissions_params = if entity_args.has_permissions {
        match entity_args.permissions_tokens.as_ref() {
            Some(tokens) => match parse_permissions_params(tokens, struct_name) {
                Ok(p) => Some(p),
                Err(e) => return e.to_compile_error().into(),
            },
            None => None,
        }
    } else {
        None
    };

    let cache_params = if entity_args.has_cache {
        match entity_args.cache_tokens.as_ref() {
            Some(tokens) => match parse_cache_params(tokens) {
                Ok(p) => Some(p),
                Err(e) => return e.to_compile_error().into(),
            },
            None => None,
        }
    } else {
        None
    };

    let audit_params = if entity_args.has_audit {
        match entity_args.audit_tokens.as_ref() {
            Some(tokens) => match parse_audit_params(tokens, struct_name) {
                Ok(p) => Some(p),
                Err(e) => return e.to_compile_error().into(),
            },
            None => None,
        }
    } else {
        None
    };

    // 生成 permissions 常量与方法
    let permissions_tokens = if let Some(ref p) = permissions_params {
        let roles: Vec<&str> = p.roles.iter().map(|s| s.as_str()).collect();
        let operations: Vec<&str> = p.operations.iter().map(|s| s.as_str()).collect();
        quote! {
            /// 允许访问此实体的角色白名单（编译期生成）
            pub const ALLOWED_ROLES: &'static [&'static str] = &[#(#roles),*];

            /// 允许对此实体执行的操作白名单（编译期生成）
            pub const ALLOWED_OPERATIONS: &'static [&'static str] = &[#(#operations),*];

            /// 获取允许的角色列表
            pub fn allowed_roles() -> &'static [&'static str] {
                Self::ALLOWED_ROLES
            }

            /// 获取允许的操作列表
            pub fn allowed_operations() -> &'static [&'static str] {
                Self::ALLOWED_OPERATIONS
            }

            /// 校验角色是否允许访问此实体
            pub fn check_permission(
                ctx: &::dbnexus::access::permission::PermissionContext,
            ) -> Result<(), ::dbnexus::DbNexusError> {
                let role = ctx.role();
                if Self::ALLOWED_ROLES.contains(&role) {
                    Ok(())
                } else {
                    Err(::dbnexus::DbNexusError::Permission(
                        ::dbnexus::domain::permission::PermissionError::Denied {
                            resource: #table_name.to_string(),
                            operation: "access".to_string(),
                        }
                    ))
                }
            }

            /// 校验角色+操作是否允许
            pub fn check_operation(
                ctx: &::dbnexus::access::permission::PermissionContext,
                action: &::dbnexus::access::permission::PermissionAction,
            ) -> Result<(), ::dbnexus::DbNexusError> {
                Self::check_permission(ctx)?;
                let op = action.to_string();
                if Self::ALLOWED_OPERATIONS.contains(&op.as_str()) {
                    Ok(())
                } else {
                    Err(::dbnexus::DbNexusError::Permission(
                        ::dbnexus::domain::permission::PermissionError::Denied {
                            resource: #table_name.to_string(),
                            operation: op,
                        }
                    ))
                }
            }
        }
    } else {
        quote! {}
    };

    // 生成 cache 常量与方法
    let cache_tokens = if let Some(ref c) = cache_params {
        let ttl = c.ttl;
        let strategy = &c.strategy;
        let max_capacity = c.max_capacity;
        quote! {
            /// 缓存 TTL（秒）
            pub const CACHE_TTL: u64 = #ttl;

            /// 缓存策略名称
            pub const CACHE_STRATEGY: &'static str = #strategy;

            /// 缓存最大容量
            pub const CACHE_MAX_CAPACITY: usize = #max_capacity;

            /// 缓存是否启用
            pub const CACHE_ENABLED: bool = true;

            /// 生成缓存键，格式为 "{table_name}:{id}"
            ///
            /// 主键类型泛型化（0.4.3），支持 i64/uuid::Uuid/String 等任何实现了 `Display` 的类型。
            pub fn cache_key<PK: ::std::fmt::Display>(id: PK) -> String {
                format!("{}:{}", #table_name, id)
            }

            /// 生成 `CacheConfig` 配置实例
            pub fn cache_config() -> ::dbnexus::foundation::config::CacheConfig {
                ::dbnexus::foundation::config::CacheConfig {
                    policy_cache_capacity: #max_capacity as u64,
                    sql_parse_cache_capacity: #max_capacity as u64,
                    query_cache_capacity: #max_capacity as u64,
                    default_ttl: #ttl,
                }
            }
        }
    } else {
        quote! {}
    };

    // 生成 audit 常量
    let audit_tokens = if let Some(ref a) = audit_params {
        let audit_table_name = &a.table_name;
        let audit_ops: Vec<&str> = a.operations.iter().map(|s| s.as_str()).collect();
        let audit_roles: Vec<&str> = a.roles.iter().map(|s| s.as_str()).collect();
        let log_values = a.log_values;
        quote! {
            /// 审计日志表名
            pub const AUDIT_TABLE_NAME: &'static str = #audit_table_name;

            /// 需要审计的操作列表
            pub const AUDIT_OPERATIONS: &'static [&'static str] = &[#(#audit_ops),*];

            /// 需要审计的角色列表
            pub const AUDIT_ROLES: &'static [&'static str] = &[#(#audit_roles),*];

            /// 是否记录变更前后的值
            pub const AUDIT_LOG_VALUES: bool = #log_values;

            /// 审计是否启用
            pub const AUDIT_ENABLED: bool = true;
        }
    } else {
        quote! {}
    };

    // Task 6.2/7.4/7.6-7.8: 生成 ActiveModelBehavior 实现
    //
    // 编排顺序（before_save 内）：validate → timestamps → user_hooks（任一失败短路）
    //
    // before_save 生成条件：validate || timestamps || has_before_save_hooks
    // after_save 生成条件：has_after_save_hooks
    // before_delete 生成条件：hooks.before_delete.is_some()
    // after_delete 生成条件：hooks.after_delete.is_some()
    //
    // - 验证：把 ActiveModel 转成 Model，调用 validator::Validate::validate
    // - 时间戳：insert 设 created_at+updated_at，update 仅设 updated_at
    // - 用户钩子：before_insert/before_update 基于 `insert` 参数分派
    //   - before_* 签名：fn(&mut ActiveModel) -> Result<(), E>
    //   - after_* 签名：fn(&Model) -> Result<(), E>
    // - 编译期签名校验（Task 7.9）：由编译器在调用点检查函数存在性和签名匹配
    let needs_before_save = entity_args.validate || entity_args.timestamps || entity_args.hooks.has_before_save_hooks();
    let needs_after_save = entity_args.hooks.has_after_save_hooks();
    let needs_before_delete = entity_args.hooks.before_delete.is_some();
    let needs_after_delete = entity_args.hooks.after_delete.is_some();
    let needs_behavior = needs_before_save || needs_after_save || needs_before_delete || needs_after_delete;

    let (behavior_attr, behavior_impl) = if needs_behavior {
        // === before_save 生成 ===
        let before_save_fn = if needs_before_save {
            // 验证逻辑（validate = true 时）
            //
            // 重要：验证步骤必须保留原始 ActiveValue 三态（Set/Unchanged/NotSet）。
            // 错误做法是 try_into_model(self) 后 __model.into() 转回 ActiveModel，
            // 因为 Model::into() 会把所有字段设为 Unchanged，丢失 Set 状态，
            // 导致 UPDATE 操作不把已修改字段包含在 SQL 中。
            //
            // 正确做法：保留 self 作为 __this，克隆 __this 用于只读验证。
            let validate_logic = if entity_args.validate {
                quote! {
                    // 验证：克隆 ActiveModel → Model → validate()，保留原始 __this 的 ActiveValue 状态
                    use ::sea_orm::TryIntoModel;
                    let mut __this = self;
                    let __model: Model = ::sea_orm::TryIntoModel::<Model>::try_into_model(__this.clone())
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                    ::validator::Validate::validate(&__model)
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                }
            } else {
                quote! { let mut __this = self; }
            };

            // 时间戳逻辑（timestamps = true 时）
            let timestamps_logic = if entity_args.timestamps {
                quote! {
                    let __now = ::time::OffsetDateTime::now_utc();
                    if insert {
                        __this.created_at = ::sea_orm::ActiveValue::Set(Some(__now));
                        __this.updated_at = ::sea_orm::ActiveValue::Set(Some(__now));
                    } else {
                        __this.updated_at = ::sea_orm::ActiveValue::Set(Some(__now));
                    }
                }
            } else {
                quote! {}
            };

            // 用户钩子逻辑（Task 7.6-7.8）
            // 编排顺序：validate → timestamps → user_hooks
            let before_insert_ident = entity_args
                .hooks
                .before_insert
                .as_ref()
                .map(|s| syn::Ident::new(s, proc_macro2::Span::call_site()));
            let before_update_ident = entity_args
                .hooks
                .before_update
                .as_ref()
                .map(|s| syn::Ident::new(s, proc_macro2::Span::call_site()));

            let before_insert_call = if let Some(ref fn_ident) = before_insert_ident {
                quote! {
                    #fn_ident(&mut __this)
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                }
            } else {
                quote! {}
            };
            let before_update_call = if let Some(ref fn_ident) = before_update_ident {
                quote! {
                    #fn_ident(&mut __this)
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                }
            } else {
                quote! {}
            };

            let hooks_logic = if entity_args.hooks.has_before_save_hooks() {
                quote! {
                    // 用户钩子（编排顺序最后）：基于 insert 参数分派
                    if insert {
                        #before_insert_call
                    } else {
                        #before_update_call
                    }
                }
            } else {
                quote! {}
            };

            quote! {
                async fn before_save<C>(
                    self,
                    db: &C,
                    insert: bool,
                ) -> Result<Self, ::sea_orm::DbErr>
                where
                    C: ::sea_orm::ConnectionTrait,
                {
                    let _ = db;
                    #validate_logic
                    #timestamps_logic
                    #hooks_logic
                    Ok(__this)
                }
            }
        } else {
            quote! {}
        };

        // === after_save 生成 ===
        // Sea-ORM 2.0 的 after_save 签名：`fn(model: Model, db, insert) -> Result<Model, DbErr>`
        // 注意：第一个参数是 Model（不是 self/ActiveModel），所以钩子可直接用 &model
        let after_save_fn = if needs_after_save {
            let after_insert_ident = entity_args
                .hooks
                .after_insert
                .as_ref()
                .map(|s| syn::Ident::new(s, proc_macro2::Span::call_site()));
            let after_update_ident = entity_args
                .hooks
                .after_update
                .as_ref()
                .map(|s| syn::Ident::new(s, proc_macro2::Span::call_site()));

            let after_insert_call = if let Some(ref fn_ident) = after_insert_ident {
                quote! {
                    #fn_ident(&model)
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                }
            } else {
                quote! {}
            };
            let after_update_call = if let Some(ref fn_ident) = after_update_ident {
                quote! {
                    #fn_ident(&model)
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                }
            } else {
                quote! {}
            };

            quote! {
                async fn after_save<C>(
                    model: Model,
                    db: &C,
                    insert: bool,
                ) -> Result<Model, ::sea_orm::DbErr>
                where
                    C: ::sea_orm::ConnectionTrait,
                {
                    let _ = db;
                    if insert {
                        #after_insert_call
                    } else {
                        #after_update_call
                    }
                    Ok(model)
                }
            }
        } else {
            quote! {}
        };

        // === before_delete 生成 ===
        let before_delete_fn = if needs_before_delete {
            let before_delete_ident = entity_args
                .hooks
                .before_delete
                .as_ref()
                .map(|s| syn::Ident::new(s, proc_macro2::Span::call_site()));

            quote! {
                async fn before_delete<C>(
                    self,
                    db: &C,
                ) -> Result<Self, ::sea_orm::DbErr>
                where
                    C: ::sea_orm::ConnectionTrait,
                {
                    let _ = db;
                    let mut __this = self;
                    #before_delete_ident(&mut __this)
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                    Ok(__this)
                }
            }
        } else {
            quote! {}
        };

        // === after_delete 生成 ===
        // Sea-ORM 2.0 的 after_delete 签名：`fn(self, db) -> Result<Self, DbErr>`
        // 钩子签名：fn(&Model) -> Result<(), E>，需将 self 转为 Model
        let after_delete_fn = if needs_after_delete {
            let after_delete_ident = entity_args
                .hooks
                .after_delete
                .as_ref()
                .map(|s| syn::Ident::new(s, proc_macro2::Span::call_site()));

            quote! {
                async fn after_delete<C>(
                    self,
                    db: &C,
                ) -> Result<Self, ::sea_orm::DbErr>
                where
                    C: ::sea_orm::ConnectionTrait,
                {
                    let _ = db;
                    use ::sea_orm::TryIntoModel;
                    let __model: Model = ::sea_orm::TryIntoModel::<Model>::try_into_model(self.clone())
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                    #after_delete_ident(&__model)
                        .map_err(|e| ::sea_orm::DbErr::Custom(e.to_string()))?;
                    Ok(self)
                }
            }
        } else {
            quote! {}
        };

        (
            quote! { #[::async_trait::async_trait] },
            quote! {
                #before_save_fn
                #after_save_fn
                #before_delete_fn
                #after_delete_fn
            },
        )
    } else {
        (quote! {}, quote! {})
    };

    // Task 6.6-6.8: soft_delete = true 时生成条件 token
    //
    // - find 方法加 `WHERE deleted_at IS NULL` 过滤（Task 6.6）
    // - delete 方法变为 `UPDATE SET deleted_at = now WHERE ... AND deleted_at IS NULL`（Task 6.7）
    // - 新增 find_with_deleted/find_only_deleted/force_delete 方法（Task 6.8）
    let soft_delete_filter = if entity_args.soft_delete {
        quote! { .filter(Column::DeletedAt.is_null()) }
    } else {
        quote! {}
    };

    // soft_delete=true 时的 delete 方法体（Task 6.7）
    let soft_delete_methods = if entity_args.soft_delete {
        quote! {
            /// 软删除：根据主键设置 deleted_at（带权限控制）
            ///
            /// 不物理删除记录，而是 UPDATE SET deleted_at = now WHERE pk = ? AND deleted_at IS NULL
            ///
            /// 主键类型由调用方决定，约束为 `Into<sea_orm::Value>`（与 sea-orm `Column::eq` 签名一致），
            /// 支持 i32/i64/String/uuid::Uuid 等任何 sea-orm `Value` 可接受的类型。
            pub async fn delete<PK>(
                session: &::dbnexus::database::pool::Session,
                pk: PK,
            ) -> Result<u64, dbnexus::DbError>
            where
                PK: Into<sea_orm::Value>,
            {
                use sea_orm::{EntityTrait, QueryFilter, QuerySelect};
                use ::sea_orm::sea_query::Expr;

                session.check_table_permission(#table_name, "UPDATE").await?;
                let conn = session.connection()?;

                let now = ::time::OffsetDateTime::now_utc();
                let result = Entity::update_many()
                    .col_expr(Column::DeletedAt, Expr::value(Some(now)))
                    .filter(Column::#primary_key_pascal.eq(pk))
                    .filter(Column::DeletedAt.is_null())
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("delete", #table_name, result.rows_affected > 0);

                Ok(result.rows_affected)
            }

            /// 软删除：批量设置 deleted_at（带权限控制）
            pub async fn delete_many(
                session: &::dbnexus::database::pool::Session,
                filter: sea_orm::Condition,
            ) -> Result<u64, dbnexus::DbError> {
                use sea_orm::{EntityTrait, QueryFilter};
                use ::sea_orm::sea_query::Expr;

                session.check_table_permission(#table_name, "UPDATE").await?;
                let conn = session.connection()?;

                let now = ::time::OffsetDateTime::now_utc();
                let result = Entity::update_many()
                    .col_expr(Column::DeletedAt, Expr::value(Some(now)))
                    .filter(filter)
                    .filter(Column::DeletedAt.is_null())
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("delete", #table_name, result.rows_affected > 0);

                Ok(result.rows_affected)
            }

            /// 查询所有记录（包括已软删除的）
            pub async fn find_with_deleted(
                session: &::dbnexus::database::pool::Session,
            ) -> Result<Vec<Self>, dbnexus::DbError> {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;

                let result = Entity::find()
                    .all(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("select", #table_name, !result.is_empty());

                Ok(result)
            }

            /// 仅查询已软删除的记录
            pub async fn find_only_deleted(
                session: &::dbnexus::database::pool::Session,
            ) -> Result<Vec<Self>, dbnexus::DbError> {
                use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;

                let result = Entity::find()
                    .filter(Column::DeletedAt.is_not_null())
                    .all(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("select", #table_name, !result.is_empty());

                Ok(result)
            }

            /// 物理删除：根据主键永久删除记录（带权限控制）
            ///
            /// 主键类型由调用方决定，约束为 `Into<sea_orm::Value>`（与 sea-orm `Column::eq` 签名一致），
            /// 支持 i32/i64/String/uuid::Uuid 等任何 sea-orm `Value` 可接受的类型。
            pub async fn force_delete<PK>(
                session: &::dbnexus::database::pool::Session,
                pk: PK,
            ) -> Result<u64, dbnexus::DbError>
            where
                PK: Into<sea_orm::Value>,
            {
                use sea_orm::{EntityTrait, QueryFilter};

                session.check_table_permission(#table_name, "DELETE").await?;
                let conn = session.connection()?;

                let result = Entity::delete_many()
                    .filter(Column::#primary_key_pascal.eq(pk))
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("delete", #table_name, result.rows_affected > 0);

                Ok(result.rows_affected)
            }
        }
    } else {
        quote! {
            /// 物理删除：根据主键删除记录（带权限控制）
            ///
            /// 主键类型由实体 `PrimaryKey::ValueType` 决定，支持 i32/i64/String/uuid::Uuid 等
            /// 任何实现了 `Into<PrimaryKey::ValueType>` 的类型。
            pub async fn delete<PK>(
                session: &::dbnexus::database::pool::Session,
                pk: PK,
            ) -> Result<u64, dbnexus::DbError>
            where
                PK: Into<<<Entity as sea_orm::EntityTrait>::PrimaryKey as sea_orm::entity::prelude::PrimaryKeyTrait>::ValueType>,
            {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "DELETE").await?;
                let conn = session.connection()?;

                let record = Entity::find_by_id(pk)
                    .one(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?
                    .ok_or_else(|| dbnexus::DbError::Config("Record not found by primary key".to_string()))?;

                let active_model: ActiveModel = record.into();
                let result = Entity::delete(active_model)
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("delete", #table_name, true);

                Ok(result.rows_affected)
            }

            /// 批量删除（带权限控制）
            pub async fn delete_many(
                session: &::dbnexus::database::pool::Session,
                filter: sea_orm::Condition,
            ) -> Result<u64, dbnexus::DbError> {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "DELETE").await?;
                let conn = session.connection()?;

                let result = Entity::delete_many()
                    .filter(filter)
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("delete", #table_name, true);

                Ok(result.rows_affected)
            }
        }
    };

    // 生成 inherent 方法 + CRUD 方法 + permissions/cache/audit
    let expanded = quote! {
        #input

        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// 获取表名
            pub fn table_name() -> &'static str {
                #table_name
            }

            /// 获取主键列名
            pub fn primary_key_column() -> &'static str {
                #primary_key_str
            }

            /// 生成实体的数据库 Schema 定义
            ///
            /// 基于 `sea_orm::Schema::create_table_from_entity` 生成 `TableCreateStatement`，
            /// 再通过 `dbnexus::domain::migration::convert_table` 转换为 dbnexus 自研的
            /// `migration::schema::Table` 结构，可直接用于 `Migration::add_table_change`。
            ///
            /// # 参数
            ///
            /// - `backend` — 数据库后端（`DbBackend::Sqlite`/`Postgres`/`MySql`），
            ///   影响列类型映射和默认值生成
            ///
            /// # 示例
            ///
            /// ```rust,ignore
            /// use dbnexus::db_entity;
            /// use sea_orm::DbBackend;
            ///
            /// let table = User::schema(DbBackend::Sqlite);
            /// migration.add_table_change(TableChange::CreateTable(table));
            /// ```
            pub fn schema(backend: ::sea_orm::DbBackend) -> ::dbnexus::domain::migration::schema::Table {
                use ::sea_orm::{EntityTrait, Schema};
                let stmt = Schema::new(backend).create_table_from_entity(Entity);
                ::dbnexus::domain::migration::convert_table(&stmt)
            }

            /// 返回 Sea-ORM 原生查询构建器（带权限检查）
            ///
            /// 用户可通过 `.filter()/.order_by()/.limit()` 等链式调用构建查询，
            /// 最后通过 `.all(conn)` 或 `.one(conn)` 执行。
            ///
            /// # 示例
            ///
            /// ```rust,ignore
            /// use sea_orm::EntityTrait;
            ///
            /// let select = User::query(&session).await?;
            /// let conn = session.connection()?;
            /// let users = select.filter(Column::Name.contains("Alice")).all(conn).await?;
            /// ```
            pub async fn query(
                session: &::dbnexus::database::pool::Session,
            ) -> Result<::sea_orm::Select<Entity>, dbnexus::DbError> {
                use ::sea_orm::EntityTrait;
                session.check_table_permission(#table_name, "SELECT").await?;
                Ok(Entity::find())
            }

            /// 返回 Sea-ORM 原生分页器（带权限检查）
            ///
            /// # 示例
            ///
            /// ```rust,ignore
            /// let paginator = User::paginate(&session, 20).await?;
            /// let total_pages = paginator.num_pages().await?;
            /// let page_data = paginator.fetch_page(0).await?;
            /// ```
            pub async fn paginate<'a>(
                session: &'a ::dbnexus::database::pool::Session,
                page_size: u64,
            ) -> Result<
                ::sea_orm::Paginator<'a, ::sea_orm::DatabaseConnection, ::sea_orm::SelectModel<Self>>,
                dbnexus::DbError,
            > {
                use ::sea_orm::{EntityTrait, PaginatorTrait};
                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;
                Ok(Entity::find().paginate(conn, page_size))
            }

            /// 批量插入记录（带权限检查）
            ///
            /// 将多个 Model 转换为 ActiveModel 后调用 `Entity::insert_many`，
            /// 返回 `InsertManyResult`（`last_insert_id` 为 `Option`）。
            ///
            /// # 示例
            ///
            /// ```rust,ignore
            /// let models = vec![user1, user2, user3];
            /// let result = User::insert_many(&session, models).await?;
            /// ```
            pub async fn insert_many(
                session: &::dbnexus::database::pool::Session,
                models: Vec<Self>,
            ) -> Result<::sea_orm::InsertManyResult<ActiveModel>, dbnexus::DbError> {
                use ::sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "INSERT").await?;
                let conn = session.connection()?;

                let active_models: Vec<ActiveModel> = models.into_iter().map(Into::into).collect();
                let result = Entity::insert_many(active_models)
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("insert_many", #table_name, true);

                Ok(result)
            }

            /// 条件批量更新（带权限检查）
            ///
            /// 使用 `Entity::update_many().filter().col_expr()` 链式调用，
            /// 返回受影响的行数。
            ///
            /// # 示例
            ///
            /// ```rust,ignore
            /// use sea_orm::ColumnTrait;
            ///
            /// let rows = User::update_many(
            ///     &session,
            ///     Column::Age.lt(18).into(),
            ///     vec![(Column::Status, "minor".into())],
            /// ).await?;
            /// ```
            pub async fn update_many(
                session: &::dbnexus::database::pool::Session,
                filter: ::sea_orm::Condition,
                updates: Vec<(Column, ::sea_orm::Value)>,
            ) -> Result<u64, dbnexus::DbError> {
                use ::sea_orm::{EntityTrait, QueryFilter};
                use ::sea_orm::sea_query::Expr;

                session.check_table_permission(#table_name, "UPDATE").await?;
                let conn = session.connection()?;

                let mut query = Entity::update_many().filter(filter);
                for (col, val) in updates {
                    query = query.col_expr(col, Expr::value(val));
                }

                let result = query
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("update_many", #table_name, result.rows_affected > 0);

                Ok(result.rows_affected)
            }

            /// CRUD 操作对应的表名
            pub const CRUD_TABLE_NAME: &'static str = #table_name;

            /// 插入新记录（带权限控制）
            ///
            /// 通过 Session 执行插入操作，自动进行权限检查和指标收集
            pub async fn insert(
                session: &::dbnexus::database::pool::Session,
                model: Self,
            ) -> Result<Self, dbnexus::DbError> {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "INSERT").await?;
                let conn = session.connection()?;

                let active_model: ActiveModel = model.into();
                let result = Entity::insert(active_model)
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("insert", #table_name, true);

                Entity::find_by_id(result.last_insert_id)
                    .one(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?
                    .ok_or_else(|| dbnexus::DbError::Config("Failed to retrieve inserted record".to_string()))
            }

            /// 根据主键查找记录（带权限控制）
            ///
            /// soft_delete=true 时自动过滤已软删除记录
            ///
            /// 主键类型由实体 `PrimaryKey::ValueType` 决定，支持 i32/i64/String/uuid::Uuid 等
            /// 任何实现了 `Into<PrimaryKey::ValueType>` 的类型均可作为 `pk` 参数。
            pub async fn find_by_id<PK>(
                session: &::dbnexus::database::pool::Session,
                pk: PK,
            ) -> Result<Option<Self>, dbnexus::DbError>
            where
                PK: Into<<<Entity as sea_orm::EntityTrait>::PrimaryKey as sea_orm::entity::prelude::PrimaryKeyTrait>::ValueType>,
            {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;

                let result = Entity::find_by_id(pk)
                    #soft_delete_filter
                    .one(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("select", #table_name, result.is_some());

                Ok(result)
            }

            /// 更新记录（带权限控制）
            ///
            /// 使用显式 `primary_key` 参数访问主键字段，不硬编码 `id`
            pub async fn update(
                session: &::dbnexus::database::pool::Session,
                model: Self,
            ) -> Result<Self, dbnexus::DbError> {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "UPDATE").await?;
                let conn = session.connection()?;

                let active_model: ActiveModel = model.into();
                // ✅ 修复 db_crud 硬编码 `id` 的 bug：使用显式 primary_key 字段名
                let primary_key = match active_model.#primary_key_ident.clone() {
                    sea_orm::ActiveValue::Set(id) => id,
                    sea_orm::ActiveValue::Unchanged(id) => id,
                    _ => return Err(dbnexus::DbError::Config("Primary key not set".to_string())),
                };

                Entity::update(active_model)
                    .exec(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("update", #table_name, true);

                Entity::find_by_id(primary_key)
                    .one(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?
                    .ok_or_else(|| dbnexus::DbError::Config("Failed to retrieve updated record".to_string()))
            }

            /// 根据主键删除记录（带权限控制）
            //
            // soft_delete=true: 软删除（UPDATE SET deleted_at）
            // soft_delete=false: 物理删除（DELETE）
            // 具体实现由 #soft_delete_methods 条件生成
            #soft_delete_methods

            /// 查询所有记录（带权限控制）
            ///
            /// soft_delete=true 时自动过滤已软删除记录
            pub async fn find_all(
                session: &::dbnexus::database::pool::Session,
            ) -> Result<Vec<Self>, dbnexus::DbError> {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;

                let result = Entity::find()
                    #soft_delete_filter
                    .all(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("select", #table_name, !result.is_empty());

                Ok(result)
            }

            /// 条件查询（带权限控制）
            ///
            /// soft_delete=true 时自动过滤已软删除记录
            pub async fn find_by_condition(
                session: &::dbnexus::database::pool::Session,
                condition: sea_orm::Condition,
            ) -> Result<Vec<Self>, dbnexus::DbError> {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;

                let result = Entity::find()
                    .filter(condition)
                    #soft_delete_filter
                    .all(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("select", #table_name, !result.is_empty());

                Ok(result)
            }

            /// 条件查询是否存在记录（带权限控制）
            ///
            /// 执行 SELECT COUNT(*) 查询，返回是否存在满足条件的记录。
            /// soft_delete=true 时自动过滤已软删除记录
            pub async fn exists(
                session: &::dbnexus::database::pool::Session,
                condition: sea_orm::Condition,
            ) -> Result<bool, dbnexus::DbError> {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;

                let count = Entity::find()
                    .filter(condition)
                    #soft_delete_filter
                    .count(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("select", #table_name, count > 0);

                Ok(count > 0)
            }

            /// 根据主键批量查找记录（带权限控制）
            ///
            /// 使用 IN 条件一次查询获取所有主键对应的记录。
            /// soft_delete=true 时自动过滤已软删除记录。
            /// 注意：返回记录的顺序可能与输入顺序不同。
            ///
            /// 主键类型由调用方决定，约束为 `Into<sea_orm::Value>`（与 sea-orm `Column::is_in` 签名一致），
            /// 支持 i32/i64/String/uuid::Uuid 等任何 sea-orm `Value` 可接受的类型。
            pub async fn find_by_ids<PK>(
                session: &::dbnexus::database::pool::Session,
                pks: Vec<PK>,
            ) -> Result<Vec<Self>, dbnexus::DbError>
            where
                PK: Into<sea_orm::Value>,
            {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;

                let result = Entity::find()
                    .filter(Column::#primary_key_pascal.is_in(pks))
                    #soft_delete_filter
                    .all(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("select", #table_name, !result.is_empty());

                Ok(result)
            }

            /// 统计记录数（带权限控制）
            ///
            /// soft_delete=true 时自动过滤已软删除记录
            pub async fn count(
                session: &::dbnexus::database::pool::Session,
            ) -> Result<u64, dbnexus::DbError> {
                use sea_orm::EntityTrait;

                session.check_table_permission(#table_name, "SELECT").await?;
                let conn = session.connection()?;

                let count = Entity::find()
                    #soft_delete_filter
                    .count(conn)
                    .await
                    .map_err(dbnexus::DbError::Connection)?;

                #[cfg(feature = "metrics")]
                session.record_metric("select", #table_name, true);

                Ok(count)
            }

            #permissions_tokens
            #cache_tokens
            #audit_tokens
        }

        // 宏生成 `impl ActiveModelBehavior for ActiveModel`
        // - timestamps=true: 生成 before_save 自动设置 created_at/updated_at（Task 6.2）
        // - validate=true: 生成 before_save 调用 validator::Validate::validate（Task 7.4）
        // - hooks(...): 生成 before_save/after_save/before_delete/after_delete 调用用户钩子（Task 7.6-7.8）
        // - 无以上参数: 空实现（保留 Sea-ORM 默认行为）
        // 用户手写此 impl 会触发 conflicting implementations 编译错误（安全失败）
        // - 注意：Sea-ORM 的 ActiveModelBehavior trait 标注了 #[async_trait]，
        //   生成任何 async fn 时，impl 块也需要 #[async_trait]
        #behavior_attr
        impl ::sea_orm::ActiveModelBehavior for ActiveModel {
            #behavior_impl
        }
    };

    TokenStream::from(expanded)
}

// ============================================================================
// 辅助函数（保留供 Phase 3 hooks/permissions/cache/audit 实现使用）
// ============================================================================

/// 验证角色名格式
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
            proc_macro_error2::abort!(
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
