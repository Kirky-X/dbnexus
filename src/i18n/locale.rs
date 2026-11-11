// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! Locale detection and global locale context.
//!
//! Provides the default-locale policy and explicit language negotiation:
//! 1. [`set_locale()`] explicit override (and the CLI `--lang` fast path)
//! 2. `DBNEXUS_LANG` environment variable (explicit, opt-in)
//! 3. `"en-US"` — the default display language is **strictly English**
//!
//! System locale detection is available only through the explicit opt-in
//! [`set_locale_from_system()`] — an implicit system locale must never make
//! the default display language Chinese on a Chinese host.
//!
//! Locale tags are normalized to the two supported display languages
//! (`"en-US"` / `"zh-CN"`); anything else fails loudly with
//! [`super::I18nError::InvalidLocale`] instead of silently rendering the
//! wrong language. The global context is async-safe (`std::sync::OnceLock` +
//! `RwLock` instead of `thread_local!`).

use std::sync::OnceLock;

/// The two supported display locales (catalog buckets `en` / `zh`).
pub const SUPPORTED_LOCALES: &[&str] = &["en-US", "zh-CN"];

/// The default display locale (strictly English).
pub const DEFAULT_LOCALE: &str = "en-US";

/// Global default locale, initialized once on first access.
static GLOBAL_LOCALE: OnceLock<&'static str> = OnceLock::new();

/// Override locale set via `set_locale()`.
static OVERRIDE_LOCALE: std::sync::RwLock<Option<&'static str>> = std::sync::RwLock::new(None);

/// Normalize a user-supplied locale string onto the supported set.
///
/// Accepts BCP-47 and POSIX spellings case-insensitively (`zh-CN`, `zh_cn`,
/// `ZH` → `"zh-CN"`; `en-US`, `en_GB`, `EN` → `"en-US"`); `@modifier` and
/// `.codeset` suffixes are stripped first. Any other language fails loudly:
/// the catalog only ships en/zh, and a silently wrong fallback must never
/// masquerade as a successful override.
///
/// # Errors
/// Returns [`super::I18nError::InvalidLocale`] for blank values, `C`/`POSIX`
/// pseudo-locales, and languages outside [`SUPPORTED_LOCALES`].
fn normalize_locale(input: &str) -> Result<&'static str, super::I18nError> {
    let stripped = input.trim();
    let core = stripped
        .split('@')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .trim();
    if core.is_empty() {
        return Err(super::I18nError::InvalidLocale {
            input: input.to_string(),
            reason: "locale tag is empty".to_string(),
        });
    }
    let normalized = core.replace('_', "-").to_lowercase();
    match normalized.as_str() {
        "c" | "posix" => Err(super::I18nError::InvalidLocale {
            input: input.to_string(),
            reason: "'C'/'POSIX' is not a display locale".to_string(),
        }),
        _ => match normalized.split('-').next().unwrap_or("") {
            "zh" => Ok("zh-CN"),
            "en" => Ok("en-US"),
            other => Err(super::I18nError::InvalidLocale {
                input: input.to_string(),
                reason: format!(
                    "unsupported display language '{other}': only en/zh catalogs are shipped"
                ),
            }),
        },
    }
}

/// Default-locale resolution for the global locale context.
///
/// **The default display language is strictly English**: the only implicit
/// inputs are an explicit `DBNEXUS_LANG` selection and, absent it, `"en-US"`.
/// System locale detection is deliberately excluded from this chain — it is
/// reachable only through the explicit opt-in [`set_locale_from_system()`],
/// so a Chinese system locale never silently turns the default display
/// language Chinese. An unusable `DBNEXUS_LANG` value (blank, or not mapping
/// onto a supported language) falls back to the strict-English default
/// rather than aborting startup.
fn detect_locale() -> &'static str {
    if let Ok(lang) = std::env::var("DBNEXUS_LANG")
        && let Ok(locale) = normalize_locale(&lang)
    {
        return locale;
    }

    DEFAULT_LOCALE
}

/// Get the current locale (a member of [`SUPPORTED_LOCALES`]).
///
/// Resolution order:
/// 1. Override set via [`set_locale()`]
/// 2. Global default (detected once on first access via `detect_locale()`)
pub fn current_locale() -> &'static str {
    if let Ok(guard) = OVERRIDE_LOCALE.read()
        && let Some(locale) = *guard
    {
        return locale;
    }

    // Fall back to global default (initialized once)
    GLOBAL_LOCALE.get_or_init(detect_locale)
}

/// Set the global locale override.
///
/// After calling this, [`current_locale()`] will return the specified locale
/// until changed again. Pass `None` to clear the override and fall back to
/// auto-detected locale.
///
/// # Errors
/// Returns [`super::I18nError::InvalidLocale`] if the locale string cannot be
/// normalized onto [`SUPPORTED_LOCALES`] (only `en*`/`zh*` display languages
/// exist; anything else fails loudly instead of silently falling back).
pub fn set_locale(locale: &str) -> Result<(), super::I18nError> {
    let normalized = normalize_locale(locale)?;

    let mut guard = OVERRIDE_LOCALE
        .write()
        .expect("locale override RwLock poisoned");
    *guard = Some(normalized);
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
/// unavailable or maps onto no supported display language.
pub fn set_locale_from_system() -> Result<(), super::I18nError> {
    let sys = sys_locale::get_locale().ok_or_else(|| super::I18nError::InvalidLocale {
        input: "(system)".to_string(),
        reason: "system locale is unavailable".to_string(),
    })?;
    set_locale(&sys)
}

/// Get the auto-detected locale (ignoring any override).
///
/// Resolves the default-locale chain (`DBNEXUS_LANG` → `"en-US"`) — it does
/// **not** consult the system locale, matching the strictly-English default
/// policy. Useful for diagnostics and logging.
pub fn detected_locale() -> &'static str {
    GLOBAL_LOCALE.get_or_init(detect_locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_locale_returns_valid_locale() {
        let locale = detect_locale();
        // Should always return a member of the supported set
        assert!(
            SUPPORTED_LOCALES.contains(&locale),
            "detected locale must be supported: {locale}"
        );
    }

    #[test]
    fn test_current_locale_returns_valid_locale() {
        // Clear any override first
        clear_locale_override();
        let locale = current_locale();
        assert!(SUPPORTED_LOCALES.contains(&locale));
    }

    #[test]
    fn test_set_locale_override() {
        set_locale("zh-CN").expect("zh-CN is valid");
        let locale = current_locale();
        assert_eq!(locale, "zh-CN");

        clear_locale_override();
        // Should return to detected locale (a supported member)
        let locale = current_locale();
        assert!(SUPPORTED_LOCALES.contains(&locale));
    }

    #[test]
    fn test_set_locale_normalizes_spelling_variants() {
        for (raw, expected) in [
            ("zh", "zh-CN"),
            ("ZH-Hant-TW", "zh-CN"),
            ("en_GB", "en-US"),
            ("en-us.utf8", "en-US"),
            ("zh_CN.UTF-8@pinyin", "zh-CN"),
        ] {
            set_locale(raw).unwrap_or_else(|_| panic!("{raw} should be valid"));
            assert_eq!(current_locale(), expected, "set_locale({raw})");
        }
        clear_locale_override();
    }

    #[test]
    fn test_set_locale_invalid() {
        for raw in [
            "not-a-valid-locale!!!",
            "",
            "  ",
            "C",
            "POSIX",
            "fr-FR",
            "ja",
        ] {
            let result = set_locale(raw);
            assert!(result.is_err(), "invalid locale '{raw}' should error");
            match result.err().unwrap() {
                super::super::I18nError::InvalidLocale { input, .. } => {
                    assert_eq!(input, raw);
                }
                other => panic!("expected InvalidLocale for '{raw}', got {other:?}"),
            }
        }
        clear_locale_override();
    }

    #[test]
    #[ignore] // 修改全局状态，并行测试时可能导致竞态
    fn test_set_locale_various_locales() {
        let locales = ["en-US", "zh-CN"];
        for locale_str in locales {
            set_locale(locale_str).unwrap_or_else(|_| panic!("{locale_str} should be valid"));
            let current = current_locale();
            assert_eq!(
                current, locale_str,
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
        // a supported member when DBNEXUS_LANG is unset or unusable — the
        // strictly-English default policy resolves to "en-US"
        // (system locale is opt-in only).
        clear_locale_override();

        let detected = detect_locale();
        assert!(
            SUPPORTED_LOCALES.contains(&detected),
            "detect_locale must return a supported locale, got {detected}"
        );
    }

    #[test]
    fn test_set_locale_from_system_applies_or_errors_explicitly() {
        clear_locale_override();
        // 显式 opt-in：系统 locale 归一到支持集时应用为覆盖；不可用或
        // 不支持时显式报错，绝不静默回退（失败显性化）。
        match set_locale_from_system() {
            Ok(()) => {
                let current = current_locale();
                assert!(
                    SUPPORTED_LOCALES.contains(&current),
                    "system locale override must land in the supported set, got {current}"
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
        assert_eq!(l1, l2, "detected_locale should be consistent across calls");
    }
}
