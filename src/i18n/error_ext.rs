// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! Extension traits for localized error messages.
//!
//! Provides [`LocalizedMsg`] (implemented by error types to declare their
//! message key and arguments) and [`I18nExt`] (blanket-implemented for all
//! errors that implement `LocalizedMsg`, providing `to_localized_string()`
//! and `message_en()`).

use super::catalog;

/// Trait for error types that have localized messages.
///
/// Each error variant returns a unique `message_key()` that maps to a
/// translation in the message catalog, and optionally provides dynamic
/// arguments via `message_args()`.
///
/// # Example
///
/// ```rust,ignore
/// use dbnexus::i18n::LocalizedMsg;
///
/// impl LocalizedMsg for MyError {
///     fn message_key(&self) -> &'static str {
///         match self {
///             MyError::NotFound { id } => "not-found",
///         }
///     }
///     fn message_args(&self) -> Vec<(&str, String)> {
///         match self {
///             MyError::NotFound { id } => vec![("id", id.clone())],
///         }
///     }
/// }
/// ```
pub trait LocalizedMsg {
    /// Return the message catalog key for this error variant.
    fn message_key(&self) -> &'static str;

    /// Return the dynamic arguments for message template substitution.
    ///
    /// Each entry is a `(name, value)` pair where `name` corresponds to
    /// a `{ $name }` placeholder in the Fluent template.
    fn message_args(&self) -> Vec<(&str, String)> {
        vec![]
    }
}

/// Extension trait providing localized string conversion for errors.
///
/// Automatically implemented for all types that implement both
/// [`LocalizedMsg`] and [`std::error::Error`].
pub trait I18nExt: LocalizedMsg + std::error::Error {
    /// Return the error message translated to the current locale.
    ///
    /// Uses the global locale context (see [`super::locale::current_locale()`])
    /// to determine the target language.
    fn to_localized_string(&self) -> String {
        catalog::translate(self.message_key(), &self.message_args())
    }

    /// Return the error message in English (the canonical fallback).
    ///
    /// This always returns the English message regardless of the current locale,
    /// using the same catalog lookup.
    fn message_en(&self) -> String {
        // Temporarily look up in English by calling translate with the key
        // The catalog's lookup_en is the canonical English source
        catalog::translate_en(self.message_key(), &self.message_args())
    }
}

// Blanket implementation for all errors that implement LocalizedMsg
impl<E: LocalizedMsg + std::error::Error> I18nExt for E {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::locale::{clear_locale_override, set_locale};
    use std::fmt;

    #[derive(Debug)]
    struct TestError;

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test error")
        }
    }

    impl std::error::Error for TestError {}

    impl LocalizedMsg for TestError {
        fn message_key(&self) -> &'static str {
            "pool-exhausted"
        }
    }

    #[test]
    fn test_to_localized_string_en() {
        set_locale("en").unwrap();
        let err = TestError;
        assert_eq!(err.to_localized_string(), "Connection pool exhausted");
    }

    #[test]
    fn test_to_localized_string_zh() {
        set_locale("zh-CN").unwrap();
        let err = TestError;
        assert_eq!(err.to_localized_string(), "连接池已耗尽");
        clear_locale_override();
    }

    #[test]
    fn test_message_en_always_english() {
        set_locale("zh-CN").unwrap();
        let err = TestError;
        // message_en should always return English
        assert_eq!(err.message_en(), "Connection pool exhausted");
        clear_locale_override();
    }

    #[test]
    fn test_to_string_still_english() {
        set_locale("zh-CN").unwrap();
        let err = TestError;
        // thiserror Display should still be English
        assert_eq!(err.to_string(), "test error");
        clear_locale_override();
    }

    #[test]
    fn test_message_key() {
        let err = TestError;
        assert_eq!(err.message_key(), "pool-exhausted");
    }

    #[test]
    fn test_message_args_default_empty() {
        let err = TestError;
        assert!(err.message_args().is_empty());
    }
}
