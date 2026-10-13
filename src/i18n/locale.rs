// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Locale detection and global locale context.
//!
//! Provides the default-locale policy and explicit language negotiation:
//! 1. [`set_locale()`] explicit override (and the CLI `--lang` fast path)
//! 2. `DBNEXUS_LANG` environment variable (explicit, opt-in)
//! 3. `"en"` — the default display language is **strictly English**
//!
//! System locale detection is available only through the explicit opt-in
//! [`set_locale_from_system()`] — an implicit system locale must never make
//! the default display language Chinese on a Chinese host.
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

/// Default-locale resolution for the global locale context.
///
/// **The default display language is strictly English**: the only implicit
/// inputs are an explicit `DBNEXUS_LANG` selection and, absent it, `"en"`.
/// System locale detection is deliberately excluded from this chain — it is
/// reachable only through the explicit opt-in [`set_locale_from_system()`],
/// so a Chinese system locale never silently turns the default display
/// language Chinese.
fn detect_locale() -> Locale {
    if let Ok(lang) = std::env::var("DBNEXUS_LANG")
        && !lang.is_empty()
        && let Ok(locale) = Locale::from_str(&lang)
    {
        return locale;
    }

    Locale::from_str("en").expect("'en' is a valid BCP-47 locale")
}

/// Get the current locale.
///
/// Resolution order:
/// 1. Override set via [`set_locale()`]
/// 2. Global default (detected once on first access via `detect_locale()`)
pub fn current_locale() -> Locale {
    if let Ok(guard) = OVERRIDE_LOCALE.read()
        && let Some(locale) = guard.as_ref()
    {
        return locale.clone();
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

    let mut guard = OVERRIDE_LOCALE
        .write()
        .expect("locale override RwLock poisoned");
    *guard = Some(parsed);
    Ok(())
}

/// Clear the locale override, reverting to auto-detected locale.
pub fn clear_locale_override() {
    let mut guard = OVERRIDE_LOCALE
        .write()
        .expect("locale override RwLock poisoned");
    *guard = None;
}

/// Explicitly adopt the system locale as the display language (opt-in).
///
/// The default locale policy is strictly English (see module docs); system
/// locale detection is preserved behind this call for callers that want
/// negotiation by host locale. Applies the detected system locale as the
/// override, so it wins over `DBNEXUS_LANG` until cleared.
///
/// # Errors
/// Returns [`super::I18nError::InvalidLocale`] if the system locale is
/// unavailable or cannot be parsed as BCP-47.
pub fn set_locale_from_system() -> Result<(), super::I18nError> {
    let sys = sys_locale::get_locale().ok_or_else(|| super::I18nError::InvalidLocale {
        input: "(system)".to_string(),
        reason: "system locale is unavailable".to_string(),
    })?;
    set_locale(&sys)
}

/// Get the auto-detected locale (ignoring any override).
///
/// Resolves the default-locale chain (`DBNEXUS_LANG` → `"en"`) — it does
/// **not** consult the system locale, matching the strictly-English default
/// policy. Useful for diagnostics and logging.
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
        assert!(
            !locale_str.is_empty(),
            "detected locale should not be empty"
        );
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
        set_locale("zh-CN").expect("zh-CN is valid");
        let locale = current_locale();
        assert_eq!(locale.to_string(), "zh-CN");

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
    #[ignore] // 修改全局状态，并行测试时可能导致竞态
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
        // the env var override path. Instead, verify detect_locale returns
        // a valid locale when DBNEXUS_LANG is unset — the strictly-English
        // default policy resolves to "en" (system locale is opt-in only).
        clear_locale_override();

        let detected = detect_locale();
        // Should return a valid locale (either DBNEXUS_LANG or "en" fallback)
        let locale_str = detected.to_string();
        assert!(
            !locale_str.is_empty(),
            "detect_locale should return a valid locale"
        );
    }

    #[test]
    fn test_set_locale_from_system_applies_or_errors_explicitly() {
        clear_locale_override();
        // 显式 opt-in：系统 locale 可用时应用为覆盖；不可用时显式报错，
        // 绝不静默回退（失败显性化）。
        match set_locale_from_system() {
            Ok(()) => {
                let current = current_locale();
                assert!(
                    !current.to_string().is_empty(),
                    "system locale override must be a valid locale"
                );
            }
            Err(super::super::I18nError::InvalidLocale { .. }) => {}
            other => panic!("expected InvalidLocale, got {other:?}"),
        }
        clear_locale_override();
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
