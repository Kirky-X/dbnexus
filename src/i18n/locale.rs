// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Locale detection and global locale context.
//!
//! Provides the locale detection priority chain:
//! 1. `DBNEXUS_LANG` environment variable
//! 2. `sys-locale` system locale detection
//! 3. `"en"` fallback
//!
//! Also provides a global locale context that is async-safe
//! (uses `std::sync::OnceLock` + `RwLock` instead of `thread_local!`).

use std::str::FromStr;
use std::sync::OnceLock;

use icu::locale::Locale;

/// Global default locale, initialized once on first access.
static GLOBAL_LOCALE: OnceLock<Locale> = OnceLock::new();

/// Override locale set via `set_locale()`.
static OVERRIDE_LOCALE: std::sync::RwLock<Option<Locale>> = std::sync::RwLock::new(None);

/// Detect the user's locale using the priority chain.
///
/// Priority: `DBNEXUS_LANG` env var → `sys-locale` → `"en"` fallback.
fn detect_locale() -> Locale {
    // 1. DBNEXUS_LANG environment variable
    if let Ok(lang) = std::env::var("DBNEXUS_LANG") {
        if !lang.is_empty() {
            if let Ok(locale) = Locale::from_str(&lang) {
                return locale;
            }
        }
    }

    // 2. sys-locale system detection
    if let Some(sys_locale) = sys_locale::get_locale() {
        if let Ok(locale) = Locale::from_str(&sys_locale) {
            return locale;
        }
    }

    // 3. English fallback
    Locale::from_str("en").expect("'en' is a valid BCP-47 locale")
}

/// Get the current locale.
///
/// Resolution order:
/// 1. Override set via [`set_locale()`]
/// 2. Global default (detected once on first access via [`detect_locale()`])
pub fn current_locale() -> Locale {
    // Check override first
    if let Ok(guard) = OVERRIDE_LOCALE.read() {
        if let Some(locale) = guard.as_ref() {
            return locale.clone();
        }
    }

    // Fall back to global default (initialized once)
    GLOBAL_LOCALE.get_or_init(detect_locale).clone()
}

/// Set the global locale override.
///
/// After calling this, [`current_locale()`] will return the specified locale
/// until changed again. Pass `None` to clear the override and fall back to
/// auto-detected locale.
///
/// # Errors
/// Returns [`super::I18nError::InvalidLocale`] if the locale string cannot be parsed.
pub fn set_locale(locale: &str) -> Result<(), super::I18nError> {
    let parsed = Locale::from_str(locale).map_err(|e| super::I18nError::InvalidLocale {
        input: locale.to_string(),
        reason: e.to_string(),
    })?;

    let mut guard = OVERRIDE_LOCALE.write().expect("locale override RwLock poisoned");
    *guard = Some(parsed);
    Ok(())
}

/// Clear the locale override, reverting to auto-detected locale.
pub fn clear_locale_override() {
    let mut guard = OVERRIDE_LOCALE.write().expect("locale override RwLock poisoned");
    *guard = None;
}

/// Get the auto-detected locale (ignoring any override).
///
/// Useful for diagnostics and logging.
pub fn detected_locale() -> Locale {
    GLOBAL_LOCALE.get_or_init(detect_locale).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_locale_returns_valid_locale() {
        let locale = detect_locale();
        // Should always return a valid locale (at minimum "en")
        let locale_str = locale.to_string();
        assert!(!locale_str.is_empty(), "detected locale should not be empty");
    }

    #[test]
    fn test_current_locale_returns_valid_locale() {
        // Clear any override first
        clear_locale_override();
        let locale = current_locale();
        assert!(!locale.to_string().is_empty());
    }

    #[test]
    fn test_set_locale_override() {
        // Set override
        set_locale("zh-CN").expect("zh-CN is valid");
        let locale = current_locale();
        assert_eq!(locale.to_string(), "zh-CN");

        // Clear override
        clear_locale_override();
        // Should return to detected locale (not necessarily "zh-CN")
        let locale = current_locale();
        assert!(!locale.to_string().is_empty());
    }

    #[test]
    fn test_set_locale_invalid() {
        let result = set_locale("not-a-valid-locale!!!");
        assert!(result.is_err(), "invalid locale should return error");
        match result.err().unwrap() {
            super::super::I18nError::InvalidLocale { input, .. } => {
                assert_eq!(input, "not-a-valid-locale!!!");
            }
            other => panic!("expected InvalidLocale, got {other:?}"),
        }
    }

    #[test]
    fn test_set_locale_various_locales() {
        let locales = ["en-US", "zh-CN"];
        for locale_str in locales {
            set_locale(locale_str).unwrap_or_else(|_| panic!("{locale_str} should be valid"));
            let current = current_locale();
            assert_eq!(
                current.to_string(),
                locale_str,
                "current_locale should return {locale_str} after set"
            );
        }
        // Clean up
        clear_locale_override();
    }

    #[test]
    fn test_clear_locale_override_when_none() {
        // Should not panic even if no override is set
        clear_locale_override();
        clear_locale_override(); // Double clear should be safe
    }

    #[test]
    fn test_detect_locale_without_env_var() {
        // Note: set_var/remove_var are unsafe in Rust 2024 edition,
        // and this crate uses #![forbid(unsafe_code)], so we can't test
        // the env var override path. Instead, verify detect_locale works
        // when DBNEXUS_LANG is not set (or falls back to sys-locale/en).
        clear_locale_override();

        let detected = detect_locale();
        // Should return a valid locale (either sys-locale or "en" fallback)
        let locale_str = detected.to_string();
        assert!(!locale_str.is_empty(), "detect_locale should return a valid locale");
    }

    #[test]
    fn test_detected_locale_consistent() {
        clear_locale_override();
        let l1 = detected_locale();
        let l2 = detected_locale();
        assert_eq!(
            l1.to_string(),
            l2.to_string(),
            "detected_locale should be consistent across calls"
        );
    }
}
