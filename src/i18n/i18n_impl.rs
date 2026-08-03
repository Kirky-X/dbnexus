// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! i18n module implementation details.
//!
//! Contains function implementations and impl blocks extracted from [`super`].

use super::*;

use std::cmp::Ordering;
use std::str::FromStr;

use icu::collator::Collator;
use icu::collator::options::CollatorOptions;
use icu::datetime::DateTimeFormatter;
use icu::datetime::fieldsets::YMD;
use icu::datetime::input::{Date, DateTime, Time};
use icu::decimal::DecimalFormatter;
use icu::decimal::input::Decimal;
use icu::decimal::options::DecimalFormatterOptions;
use icu::locale::Locale;
use icu::plurals::{PluralCategory, PluralRules, PluralRulesOptions};
use writeable::Writeable;

/// Map a [`PluralCategory`] to its capitalized CLDR name (e.g. `"One"`, `"Other"`).
fn plural_category_name(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "Zero",
        PluralCategory::One => "One",
        PluralCategory::Two => "Two",
        PluralCategory::Few => "Few",
        PluralCategory::Many => "Many",
        PluralCategory::Other => "Other",
    }
}

// ============================================
// 消息目录（Message Catalog）
// ============================================

/// Get a locale-specific message template by key.
///
/// Returns the translated template string with `{count}` placeholders
/// where applicable. Falls back to English for unsupported locales.
fn get_message(locale: &Locale, key: &str) -> &'static str {
    let lang = locale.id.language.as_str();
    match (lang, key) {
        // --- 迁移消息 ---
        ("zh", "migration") => "已应用 {count} 个迁移",
        ("de", "migration") => "{count} Migrationen angewendet",
        ("ja", "migration") => "{count} 件のマイグレーションを適用しました",
        ("fr", "migration") => "{count} migrations appliquées",
        // English and fallback
        (_, "migration") => "{count} migrations applied",

        // --- 通用消息 ---
        ("zh", "hello_world") => "你好，世界！",
        ("de", "hello_world") => "Hallo, Welt!",
        ("ja", "hello_world") => "こんにちは、世界！",
        ("fr", "hello_world") => "Bonjour, le monde !",
        (_, "hello_world") => "Hello, World!",

        // Unknown key fallback
        _ => "",
    }
}

/// Replace `{count}` placeholder in a message template with a formatted string.
fn substitute_count(template: &str, formatted_count: &str) -> String {
    template.replace("{count}", formatted_count)
}

impl DbI18nFormatter {
    /// Create a new formatter for the given BCP-47 locale tag.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidLocale`] if the tag cannot be parsed,
    /// or [`I18nError::FormatError`] if ICU4X lacks compiled data for it.
    pub fn new(locale: &str) -> Result<Self, I18nError> {
        let parsed = Locale::from_str(locale).map_err(|e| I18nError::InvalidLocale {
            input: locale.to_string(),
            reason: e.to_string(),
        })?;

        let decimal_formatter = DecimalFormatter::try_new(parsed.clone().into(), DecimalFormatterOptions::default())
            .map_err(|e| I18nError::FormatError(e.to_string()))?;

        let plural_rules = PluralRules::try_new(parsed.clone().into(), PluralRulesOptions::default())
            .map_err(|e| I18nError::FormatError(e.to_string()))?;

        let collator = Collator::try_new(parsed.clone().into(), CollatorOptions::default())
            .map_err(|e| I18nError::FormatError(e.to_string()))?;

        Ok(Self {
            locale: parsed,
            decimal_formatter,
            plural_rules,
            collator,
        })
    }

    /// Format a floating-point number with locale-sensitive grouping
    /// and decimal separators.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] for non-finite values or
    /// if the value cannot be parsed into a fixed decimal.
    pub fn format_number(&self, value: f64) -> Result<String, I18nError> {
        if !value.is_finite() {
            return Err(I18nError::InvalidNumber {
                input: value.to_string(),
                reason: "value is not finite (NaN or Infinity)".into(),
            });
        }
        let repr = format!("{value}");
        let decimal = Decimal::from_str(&repr).map_err(|e| I18nError::InvalidNumber {
            input: repr,
            reason: e.to_string(),
        })?;
        let formatted = self.decimal_formatter.format(&decimal);
        Ok(formatted.write_to_string().into_owned())
    }

    /// Format a row count with locale-sensitive grouping separators
    /// (e.g. `"1,234,567"` for en-US).
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] if the count cannot be formatted.
    pub fn format_row_count(&self, count: u64) -> Result<String, I18nError> {
        self.format_number(count as f64)
    }

    /// Build a locale-aware migration message combining the formatted
    /// count with a locale-specific translated template
    /// (e.g. `"1 migration applied"` for en-US, `"已应用 1 个迁移"` for zh-CN).
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] if the count cannot be formatted.
    pub fn format_migration_message(&self, count: u64) -> Result<String, I18nError> {
        let count_str = self.format_number(count as f64)?;
        let template = get_message(&self.locale, "migration");
        Ok(substitute_count(template, &count_str))
    }

    /// Format an ISO calendar date (year / month / day) as a migration
    /// timestamp using a medium-length locale-specific pattern.
    ///
    /// # Errors
    /// Returns [`I18nError::DateError`] if any component is out of range,
    /// or [`I18nError::FormatError`] if the formatter cannot be constructed.
    pub fn format_timestamp(&self, year: i32, month: u8, day: u8) -> Result<String, I18nError> {
        let date = Date::try_new_iso(year, month, day).map_err(|e| I18nError::DateError(e.to_string()))?;
        let time = Time::try_new(0, 0, 0, 0).map_err(|e| I18nError::DateError(e.to_string()))?;
        let datetime = DateTime { date, time };

        let dtf = DateTimeFormatter::try_new(self.locale.clone().into(), YMD::medium())
            .map_err(|e| I18nError::FormatError(e.to_string()))?;
        let formatted = dtf.format(&datetime);
        Ok(formatted.write_to_string().into_owned())
    }

    /// Return the plural category name for `count` in the formatter's locale
    /// (e.g. `"One"` for English count=1, `"Other"` for count=2).
    ///
    /// # Errors
    /// This method does not currently fail, but returns `Result` for API
    /// consistency with the other formatting methods.
    pub fn plural_category(&self, count: u64) -> Result<String, I18nError> {
        Ok(plural_category_name(self.plural_rules.category_for(count)).to_string())
    }

    /// Compare two strings (e.g. SQL error messages) using locale-sensitive
    /// collation rules.
    ///
    /// # Errors
    /// This method does not currently fail, but returns `Result` for API
    /// consistency with the other formatting methods.
    pub fn compare_strings(&self, a: &str, b: &str) -> Result<Ordering, I18nError> {
        Ok(self.collator.compare(a, b))
    }
}
