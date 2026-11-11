// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! i18n module implementation details.
//!
//! [`DbI18nFormatter`] delegates all ICU4X formatting (number / date /
//! plural / collation) to the trait-kit `I18nFormatter` (the shared i18n
//! formatting layer of the kit ecosystem); dbnexus keeps only the SQL-facing
//! conveniences (`format_row_count`, `format_migration_message`) and its own
//! stable error type.

use std::cmp::Ordering;

use trait_kit::i18n::I18nFormatter;

use super::DbI18nFormatter;
use super::I18nError;
use super::catalog;

/// Convert a trait-kit i18n error into the dbnexus error type
/// (same variant shapes, one-to-one).
impl From<trait_kit::i18n::I18nError> for I18nError {
    fn from(err: trait_kit::i18n::I18nError) -> Self {
        use trait_kit::i18n::I18nError as Tk;
        match err {
            Tk::InvalidLocale { input, reason } => Self::InvalidLocale { input, reason },
            Tk::InvalidNumber { input, reason } => Self::InvalidNumber { input, reason },
            Tk::DateError(detail) => Self::DateError(detail),
            Tk::FormatError(detail) => Self::FormatError(detail),
        }
    }
}

/// Map a plural category to its capitalized CLDR name (e.g. `"One"`,
/// `"Other"`).
///
/// The category enum (`icu_plurals::PluralCategory`) is not re-exported by
/// trait-kit, so the variant name is taken from its `Debug` output — the
/// variants are exactly the capitalized CLDR category names
/// (`Zero`/`One`/`Two`/`Few`/`Many`/`Other`), a stable icu_plurals surface.
fn plural_category_name<C: std::fmt::Debug>(category: C) -> String {
    format!("{category:?}")
}

// 消息目录（Message Catalog）

// 同一语义的迁移消息只在 FTL 目录（catalog.rs 的 `migration` 键）保留一份：
// format_migration_message 经 catalog::format_from_bundle 取该键，
// 避免此处再维护第二套占位符语法不同的模板目录。

impl DbI18nFormatter {
    /// Create a new formatter for the given BCP-47 locale tag.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidLocale`] if the tag cannot be parsed,
    /// or [`I18nError::FormatError`] if ICU4X lacks compiled data for it.
    pub fn new(locale: &str) -> Result<Self, I18nError> {
        let inner = I18nFormatter::new(locale).map_err(I18nError::from)?;
        Ok(Self {
            inner,
            locale_tag: locale.to_string(),
        })
    }

    /// Format a floating-point number with locale-sensitive grouping
    /// and decimal separators.
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] for non-finite values or
    /// if the value cannot be parsed into a fixed decimal.
    pub fn format_number(&self, value: f64) -> Result<String, I18nError> {
        self.inner.format_number(value).map_err(I18nError::from)
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
    /// count with the FTL catalog's `migration` template
    /// (e.g. `"5 migrations applied"` for en, `"已应用 5 个迁移"` for zh-CN).
    ///
    /// Falls back to the bare key if the template is missing
    /// (same last-resort contract as [`super::translate`]).
    ///
    /// # Errors
    /// Returns [`I18nError::InvalidNumber`] if the count cannot be formatted.
    pub fn format_migration_message(&self, count: u64) -> Result<String, I18nError> {
        let count_str = self.format_number(count as f64)?;
        Ok(catalog::format_from_bundle(
            self.language_bucket(),
            "migration",
            &[("count", count_str)],
        )
        .unwrap_or_else(|| "migration".to_string()))
    }

    /// Format an ISO calendar date (year / month / day) as a migration
    /// timestamp using a medium-length locale-specific pattern.
    ///
    /// # Errors
    /// Returns [`I18nError::DateError`] if any component is out of range,
    /// or [`I18nError::FormatError`] if the formatter cannot be constructed.
    pub fn format_timestamp(&self, year: i32, month: u8, day: u8) -> Result<String, I18nError> {
        self.inner
            .format_date(year, month, day)
            .map_err(I18nError::from)
    }

    /// Return the plural category name for `count` in the formatter's locale
    /// (e.g. `"One"` for English count=1, `"Other"` for count=2).
    ///
    /// # Errors
    /// This method does not currently fail, but returns `Result` for API
    /// consistency with the other formatting methods.
    pub fn plural_category(&self, count: u64) -> Result<String, I18nError> {
        let category = self.inner.plural_category(count).map_err(I18nError::from)?;
        Ok(plural_category_name(category))
    }

    /// Compare two strings (e.g. SQL error messages) using locale-sensitive
    /// collation rules.
    ///
    /// # Errors
    /// This method does not currently fail, but returns `Result` for API
    /// consistency with the other formatting methods.
    pub fn compare_strings(&self, a: &str, b: &str) -> Result<Ordering, I18nError> {
        self.inner.compare(a, b).map_err(I18nError::from)
    }

    /// The catalog language bucket (`"zh"` / `"en"`) this formatter resolves
    /// messages for.
    fn language_bucket(&self) -> &'static str {
        if self.locale_tag.to_lowercase().starts_with("zh") {
            "zh"
        } else {
            "en"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plural_category_name_all_variants() {
        // Arabic (ar) has Zero category for count=0
        let fmt_ar = DbI18nFormatter::new("ar").expect("ar locale");
        let cat = fmt_ar.plural_category(0).expect("ar plural 0");
        assert_eq!(cat, "Zero", "Arabic count=0 should be Zero, got: {cat}");

        // English count=1 → One
        let fmt_en = DbI18nFormatter::new("en").expect("en locale");
        assert_eq!(fmt_en.plural_category(1).unwrap(), "One");

        // Some locales have Two (e.g. Arabic count=2)
        let cat2 = fmt_ar.plural_category(2).expect("ar plural 2");
        assert_eq!(cat2, "Two", "Arabic count=2 should be Two, got: {cat2}");

        // Polish (pl) has Few for count=2,3,4
        let fmt_pl = DbI18nFormatter::new("pl").expect("pl locale");
        let cat_few = fmt_pl.plural_category(2).expect("pl plural 2");
        assert_eq!(
            cat_few, "Few",
            "Polish count=2 should be Few, got: {cat_few}"
        );

        // Many category: Arabic has Many for values 11..99 (e.g. 11)
        let cat_many = fmt_ar.plural_category(11).expect("ar plural 11");
        assert_eq!(
            cat_many, "Many",
            "Arabic count=11 should be Many, got: {cat_many}"
        );

        // Other: English count=0
        let cat_other = fmt_en.plural_category(0).expect("en plural 0");
        assert_eq!(
            cat_other, "Other",
            "English count=0 should be Other, got: {cat_other}"
        );
    }

    #[test]
    fn test_get_message_via_migration_all_locales() {
        let locales_and_expected = vec![("zh-CN", "迁移"), ("en-US", "migrations applied")];
        for (locale, expected) in locales_and_expected {
            let fmt = DbI18nFormatter::new(locale).unwrap_or_else(|_| panic!("locale {locale}"));
            let msg = fmt
                .format_migration_message(5)
                .unwrap_or_else(|_| panic!("migration msg for {locale}"));
            assert!(
                msg.contains(expected),
                "locale {locale}: expected '{expected}' in '{msg}'"
            );
        }
    }

    #[test]
    fn test_format_number_zh() {
        let fmt = DbI18nFormatter::new("zh-CN").expect("zh-CN locale");
        let result = fmt.format_number(1234567.0).expect("format number zh");
        // Chinese uses different grouping separator
        assert!(!result.is_empty());
    }

    #[test]
    fn test_format_timestamp_invalid_date() {
        let fmt = DbI18nFormatter::new("en-US").expect("en-US locale");
        // Month 13 is invalid
        let result = fmt.format_timestamp(2026, 13, 1);
        assert!(result.is_err(), "invalid month should return error");
    }

    #[test]
    fn test_format_timestamp_invalid_day() {
        let fmt = DbI18nFormatter::new("en-US").expect("en-US locale");
        // Day 32 is invalid
        let result = fmt.format_timestamp(2026, 1, 32);
        assert!(result.is_err(), "invalid day should return error");
    }

    #[test]
    fn test_format_migration_message_fr() {
        let fmt = DbI18nFormatter::new("fr-FR").expect("fr-FR locale");
        let msg = fmt
            .format_migration_message(10)
            .expect("fr migration message");
        assert!(msg.contains("migrations"), "fr message: got '{msg}'");
    }
}
