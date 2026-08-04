// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! ICU4X-backed internationalization for database operations.
//!
//! Provides:
//! - **Locale detection**: Automatic system locale detection with `DBNEXUS_LANG`
//!   environment variable override and manual `set_locale()` API.
//! - **Message catalog**: Compile-time embedded translations for 2 languages
//!   (en, zh) covering all error types and CLI output.
//! - **LocalizedMsg / I18nExt traits**: Extension traits for error types to
//!   provide `to_localized_string()` and `message_en()`.
//! - **DbI18nFormatter**: ICU4X locale-aware number/date/plural/collation formatting.
//!
//! This is a **core feature** — always available, no feature flag required.
//!
//! # Locale Priority
//!
//! 1. `set_locale()` override
//! 2. `DBNEXUS_LANG` environment variable
//! 3. `sys-locale` system detection
//! 4. `"en"` fallback
//!
//! # Example
//!
//! ```rust,ignore
//! use dbnexus::i18n;
//!
//! // Set locale
//! i18n::set_locale("zh-CN").unwrap();
//!
//! // Translate a message
//! let msg = i18n::t("pool-exhausted", &[]);
//! // => "连接池已耗尽"
//!
//! // Use DbI18nFormatter for number/date formatting
//! let fmt = i18n::DbI18nFormatter::new("en-US")?;
//! let rows = fmt.format_row_count(1_234_567)?;
//! ```

pub mod catalog;
pub mod error_ext;
mod i18n_impl;
pub mod locale;

// Re-export commonly used functions at module level
#[allow(unused_imports)]
pub use catalog::{t, t_simple, translate};
#[allow(unused_imports)]
pub use locale::{current_locale, set_locale};

use icu::collator::CollatorBorrowed;
use icu::decimal::DecimalFormatter;
use icu::locale::Locale;
use icu::plurals::PluralRules;
use thiserror::Error;

/// Errors returned by [`DbI18nFormatter`] operations.
#[derive(Debug, Error)]
pub enum I18nError {
    /// BCP-47 locale string could not be parsed.
    #[error("invalid locale '{input}': {reason}")]
    InvalidLocale {
        /// The locale string that failed to parse.
        input: String,
        /// The parse error reason.
        reason: String,
    },
    /// Number value could not be formatted (e.g. NaN, Infinity, or parse failure).
    #[error("invalid number '{input}': {reason}")]
    InvalidNumber {
        /// The number string that failed to format.
        input: String,
        /// The formatting error reason.
        reason: String,
    },
    /// Date component out of range or otherwise invalid.
    #[error("date error: {0}")]
    DateError(String),
    /// Underlying ICU4X data or formatting failure.
    #[error("formatting error: {0}")]
    FormatError(String),
}

/// Locale-aware formatter backed by ICU4X compiled data.
///
/// Construct with [`DbI18nFormatter::new`] using a BCP-47 locale tag
/// (e.g. `"en-US"`, `"zh-CN"`). All formatters are created eagerly so
/// that repeated formatting calls are allocation-light.
pub struct DbI18nFormatter {
    locale: Locale,
    decimal_formatter: DecimalFormatter,
    plural_rules: PluralRules,
    collator: CollatorBorrowed<'static>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_locale_parsing_en() {
        let fmt = DbI18nFormatter::new("en-US");
        assert!(fmt.is_ok(), "en-US should parse successfully");
    }

    #[test]
    fn test_locale_parsing_zh() {
        let fmt = DbI18nFormatter::new("zh-CN");
        assert!(fmt.is_ok(), "zh-CN should parse successfully");
    }

    #[test]
    fn test_invalid_locale() {
        let result = DbI18nFormatter::new("not-a-valid-locale!!!");
        assert!(result.is_err(), "invalid locale should return error");
        match result.err().unwrap() {
            I18nError::InvalidLocale { input, .. } => assert_eq!(input, "not-a-valid-locale!!!"),
            other => panic!("expected InvalidLocale, got {other:?}"),
        }
    }

    #[test]
    fn test_format_migration_message_en() {
        let fmt = DbI18nFormatter::new("en").expect("en locale");
        let msg = fmt.format_migration_message(1).expect("migration message");
        assert!(msg.contains("1"), "message should contain count: got '{msg}'");
        assert!(
            msg.contains("migration") && msg.contains("applied"),
            "en migration message: got '{msg}'"
        );

        let msg2 = fmt.format_migration_message(5).expect("migration message");
        assert!(msg2.contains("5"), "message should contain count: got '{msg2}'");
    }

    #[test]
    fn test_format_migration_message_zh() {
        let fmt = DbI18nFormatter::new("zh-CN").expect("zh-CN locale");
        let msg = fmt.format_migration_message(1).expect("migration message");
        assert!(
            msg.contains("已应用") && msg.contains("迁移"),
            "zh-CN migration message should contain Chinese text: got '{msg}'"
        );
        assert!(msg.contains("1"), "message should contain count: got '{msg}'");
    }

    #[test]
    fn test_format_row_count() {
        let fmt = DbI18nFormatter::new("en-US").expect("en-US locale");
        let result = fmt.format_row_count(1_234_567).expect("row count");
        assert!(
            result.contains(','),
            "en-US row count should contain thousands separator: got '{result}'"
        );
    }

    #[test]
    fn test_format_number_en() {
        let fmt = DbI18nFormatter::new("en-US").expect("en-US locale");
        let result = fmt.format_number(1_234_567.89_f64).expect("format number");
        assert!(
            result.contains(','),
            "en-US number should contain thousands separator: got '{result}'"
        );
        assert!(
            result.contains('.'),
            "en-US number should contain decimal point: got '{result}'"
        );
    }

    #[test]
    fn test_format_number_not_finite() {
        let fmt = DbI18nFormatter::new("en-US").expect("en-US locale");
        assert!(fmt.format_number(f64::NAN).is_err());
        assert!(fmt.format_number(f64::INFINITY).is_err());
    }

    #[test]
    fn test_plural_category() {
        let fmt = DbI18nFormatter::new("en").expect("en locale");
        assert_eq!(
            fmt.plural_category(1).expect("plural 1"),
            "One",
            "en: count=1 should be One"
        );
        assert_eq!(
            fmt.plural_category(2).expect("plural 2"),
            "Other",
            "en: count=2 should be Other"
        );
    }

    #[test]
    fn test_format_timestamp() {
        let fmt = DbI18nFormatter::new("en-US").expect("en-US locale");
        let result = fmt.format_timestamp(2026, 7, 11).expect("timestamp");
        assert!(result.contains("2026"), "timestamp should contain year: got '{result}'");
        assert!(!result.is_empty(), "timestamp should be non-empty: got '{result}'");
    }

    #[test]
    fn test_compare_strings() {
        let fmt = DbI18nFormatter::new("en").expect("en locale");
        assert_eq!(
            fmt.compare_strings("apple", "banana").expect("compare"),
            Ordering::Less,
            "apple < banana"
        );
        assert_eq!(
            fmt.compare_strings("banana", "apple").expect("compare"),
            Ordering::Greater,
            "banana > apple"
        );
        assert_eq!(
            fmt.compare_strings("apple", "apple").expect("compare"),
            Ordering::Equal,
            "apple == apple"
        );
    }
}
