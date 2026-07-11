// Copyright (c) 2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 敏感数据处理模块
//!
//! 提供数据脱敏和加密存储功能，包括：
//! - 数据脱敏：手机号、邮箱、身份证、银行卡等
//! - 安全哈希：使用 SHA-256 或 bcrypt 进行不可逆哈希
//! - AES 加密：可逆加密用于敏感数据存储

use thiserror::Error;

/// 敏感数据处理错误
#[derive(Debug, Error)]
pub enum SensitiveError {
    /// 脱敏失败
    #[error("Masking failed: {0}")]
    MaskingFailed(String),

    /// 加密失败
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    /// 解密失败
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    /// 无效的密钥
    #[error("Invalid key: {0}")]
    InvalidKey(String),

    /// 无效的输入数据
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// 敏感数据处理结果
pub type SensitiveResult<T> = Result<T, SensitiveError>;

/// 数据脱敏类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskType {
    /// 手机号脱敏（保留前3后4）
    Phone,
    /// 邮箱脱敏（保留前2字符和域名）
    Email,
    /// 身份证脱敏（保留前4后4）
    IdCard,
    /// 银行卡脱敏（保留前4后4）
    BankCard,
    /// 姓名脱敏（保留姓氏）
    Name,
    /// 地址脱敏（保留省市）
    Address,
    /// 自定义脱敏（指定保留前后位数）
    Custom {
        /// 保留前几位
        keep_prefix: usize,
        /// 保留后几位
        keep_suffix: usize,
    },
}

/// 敏感数据脱敏器
pub struct SensitiveMasker;

impl SensitiveMasker {
    /// 创建新的脱敏器
    pub fn new() -> Self {
        Self
    }

    /// 对敏感数据进行脱敏
    ///
    /// # 参数
    ///
    /// * `data` - 原始敏感数据
    /// * `mask_type` - 脱敏类型
    ///
    /// # 返回
    ///
    /// 脱敏后的字符串
    pub fn mask(data: &str, mask_type: MaskType) -> SensitiveResult<String> {
        let data = data.trim();

        match mask_type {
            MaskType::Phone => Self::mask_phone(data),
            MaskType::Email => Self::mask_email(data),
            MaskType::IdCard => Self::mask_id_card(data),
            MaskType::BankCard => Self::mask_bank_card(data),
            MaskType::Name => Self::mask_name(data),
            MaskType::Address => Self::mask_address(data),
            MaskType::Custom {
                keep_prefix,
                keep_suffix,
            } => Self::mask_custom(data, keep_prefix, keep_suffix),
        }
    }

    /// 手机号脱敏：138****1234
    fn mask_phone(phone: &str) -> SensitiveResult<String> {
        // 移除可能的空格和横线
        let cleaned: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

        if cleaned.len() < 7 {
            return Err(SensitiveError::InvalidInput("Phone number too short".to_string()));
        }

        let chars: Vec<char> = cleaned.chars().collect();
        let len = chars.len();

        // 保留前3后4
        let prefix: String = chars[..3].iter().collect();
        let suffix: String = chars[len - 4..].iter().collect();
        let mask_count = len - 7;

        Ok(format!("{}{}{}", prefix, "*".repeat(mask_count), suffix))
    }

    /// 邮箱脱敏：ab***@example.com
    ///
    /// 使用 `chars()` 而非字节切片处理本地部分，确保非 ASCII 字符（中文、emoji 等）
    /// 不会触发字节边界 panic。
    fn mask_email(email: &str) -> SensitiveResult<String> {
        let parts: Vec<&str> = email.split('@').collect();

        if parts.len() != 2 {
            return Err(SensitiveError::InvalidInput("Invalid email format".to_string()));
        }

        let local = parts[0];
        let domain = parts[1];

        if local.is_empty() {
            return Err(SensitiveError::InvalidInput("Email local part is empty".to_string()));
        }

        // 使用 chars() 安全处理 Unicode（避免非 ASCII 字节切片 panic）
        let local_chars: Vec<char> = local.chars().collect();
        let prefix_len = local_chars.len().min(2);
        let prefix: String = local_chars[..prefix_len].iter().collect();
        let mask_count = local_chars.len() - prefix_len;

        Ok(format!("{}{}@{}", prefix, "*".repeat(mask_count.max(3)), domain))
    }

    /// 身份证脱敏：1101************1234
    fn mask_id_card(id: &str) -> SensitiveResult<String> {
        // 移除可能的空格和横线
        let cleaned: String = id
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x')
            .collect();

        // 身份证号可以是15位或18位
        if cleaned.len() != 15 && cleaned.len() != 18 {
            return Err(SensitiveError::InvalidInput("Invalid ID card length".to_string()));
        }

        let chars: Vec<char> = cleaned.chars().collect();
        let len = chars.len();

        // 保留前4后4
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[len - 4..].iter().collect();
        let mask_count = len - 8;

        Ok(format!("{}{}{}", prefix, "*".repeat(mask_count), suffix))
    }

    /// 银行卡脱敏：6222****1234
    fn mask_bank_card(card: &str) -> SensitiveResult<String> {
        // 移除可能的空格
        let cleaned: String = card.chars().filter(|c| c.is_ascii_digit()).collect();

        if cleaned.len() < 8 {
            return Err(SensitiveError::InvalidInput("Bank card number too short".to_string()));
        }

        let chars: Vec<char> = cleaned.chars().collect();
        let len = chars.len();

        // 保留前4后4
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[len - 4..].iter().collect();
        let mask_count = len - 8;

        Ok(format!("{}{}{}", prefix, "*".repeat(mask_count), suffix))
    }

    /// 姓名脱敏：张**
    fn mask_name(name: &str) -> SensitiveResult<String> {
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(SensitiveError::InvalidInput("Name is empty".to_string()));
        }

        let chars: Vec<char> = trimmed.chars().collect();
        let len = chars.len();

        // 保留第一个字符（姓氏）
        let surname: String = chars[..1].iter().collect();
        let mask_count = len - 1;

        Ok(format!("{}{}", surname, "*".repeat(mask_count)))
    }

    /// 地址脱敏：保留省市区，其余用*代替
    fn mask_address(address: &str) -> SensitiveResult<String> {
        let trimmed = address.trim();

        if trimmed.is_empty() {
            return Err(SensitiveError::InvalidInput("Address is empty".to_string()));
        }

        // 尝试找到省市区关键词
        let keywords = ["省", "市", "区", "县", "镇"];

        // 找到最后一个行政区划关键词的位置
        let mut last_keyword_pos = 0;
        for keyword in &keywords {
            if let Some(pos) = trimmed.find(keyword) {
                last_keyword_pos = last_keyword_pos.max(pos + keyword.len());
            }
        }

        // 如果找到行政区划，保留到该位置
        if last_keyword_pos > 0 && last_keyword_pos < trimmed.len() {
            let prefix = &trimmed[..last_keyword_pos];
            let mask_len = trimmed.len() - last_keyword_pos;
            return Ok(format!("{}{}", prefix, "*".repeat(mask_len)));
        }

        // 否则保留前6个字符
        let chars: Vec<char> = trimmed.chars().collect();
        let prefix_len = chars.len().min(6);
        let prefix: String = chars[..prefix_len].iter().collect();
        let mask_len = chars.len() - prefix_len;

        Ok(format!("{}{}", prefix, "*".repeat(mask_len)))
    }

    /// 自定义脱敏
    fn mask_custom(data: &str, keep_prefix: usize, keep_suffix: usize) -> SensitiveResult<String> {
        if data.is_empty() {
            return Err(SensitiveError::InvalidInput("Data is empty".to_string()));
        }

        let chars: Vec<char> = data.chars().collect();
        let len = chars.len();

        if keep_prefix + keep_suffix >= len {
            // 如果保留位数超过总长度，返回原数据
            return Ok(data.to_string());
        }

        let prefix: String = chars[..keep_prefix].iter().collect();
        let suffix: String = chars[len - keep_suffix..].iter().collect();
        let mask_count = len - keep_prefix - keep_suffix;

        Ok(format!("{}{}{}", prefix, "*".repeat(mask_count), suffix))
    }
}

impl Default for SensitiveMasker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_phone() {
        // 标准11位手机号
        assert_eq!(
            SensitiveMasker::mask("13812345678", MaskType::Phone).unwrap(),
            "138****5678"
        );

        // 带空格的手机号
        assert_eq!(
            SensitiveMasker::mask("138 1234 5678", MaskType::Phone).unwrap(),
            "138****5678"
        );

        // 带横线的手机号
        assert_eq!(
            SensitiveMasker::mask("138-1234-5678", MaskType::Phone).unwrap(),
            "138****5678"
        );
    }

    #[test]
    fn test_mask_email() {
        // 标准邮箱
        let result = SensitiveMasker::mask("test@example.com", MaskType::Email).unwrap();
        assert!(result.starts_with("te"));
        assert!(result.ends_with("@example.com"));
        assert!(result.contains('*'));

        // 短邮箱
        let result = SensitiveMasker::mask("a@b.com", MaskType::Email).unwrap();
        assert!(result.contains('*'));
    }

    #[test]
    fn test_mask_id_card() {
        // 18位身份证
        let result = SensitiveMasker::mask("110101199001011234", MaskType::IdCard).unwrap();
        assert_eq!(&result[..4], "1101");
        assert_eq!(&result[result.len() - 4..], "1234");
        assert!(result.contains('*'));

        // 15位身份证
        let result = SensitiveMasker::mask("110101900101123", MaskType::IdCard).unwrap();
        assert_eq!(&result[..4], "1101");
        assert_eq!(&result[result.len() - 4..], "1123");
    }

    #[test]
    fn test_mask_bank_card() {
        // 16位银行卡
        let result = SensitiveMasker::mask("6222021234567890", MaskType::BankCard).unwrap();
        assert_eq!(&result[..4], "6222");
        assert_eq!(&result[result.len() - 4..], "7890");
        assert!(result.contains('*'));

        // 19位银行卡
        let result = SensitiveMasker::mask("6222021234567890123", MaskType::BankCard).unwrap();
        assert_eq!(&result[..4], "6222");
        assert_eq!(&result[result.len() - 4..], "0123");
    }

    #[test]
    fn test_mask_name() {
        // 两个字姓名
        assert_eq!(SensitiveMasker::mask("张三", MaskType::Name).unwrap(), "张*");

        // 三个字姓名
        assert_eq!(SensitiveMasker::mask("李某某", MaskType::Name).unwrap(), "李**");

        // 四个字姓名
        assert_eq!(SensitiveMasker::mask("欧阳明月", MaskType::Name).unwrap(), "欧***");
    }

    #[test]
    fn test_mask_address() {
        // 带省市区
        let result = SensitiveMasker::mask("北京市朝阳区某某街道123号", MaskType::Address).unwrap();
        assert!(result.starts_with("北京市朝阳区"));
        assert!(result.contains('*'));

        // 不带行政区划
        let result = SensitiveMasker::mask("某某街道123号", MaskType::Address).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_mask_custom() {
        // 自定义保留前后位数
        assert_eq!(
            SensitiveMasker::mask(
                "1234567890",
                MaskType::Custom {
                    keep_prefix: 2,
                    keep_suffix: 2
                }
            )
            .unwrap(),
            "12******90"
        );

        // 保留位数超过总长度
        assert_eq!(
            SensitiveMasker::mask(
                "123",
                MaskType::Custom {
                    keep_prefix: 2,
                    keep_suffix: 2
                }
            )
            .unwrap(),
            "123"
        );
    }

    #[test]
    fn test_invalid_inputs() {
        // 无效手机号
        assert!(SensitiveMasker::mask("123", MaskType::Phone).is_err());

        // 无效邮箱
        assert!(SensitiveMasker::mask("invalid", MaskType::Email).is_err());

        // 无效身份证
        assert!(SensitiveMasker::mask("12345", MaskType::IdCard).is_err());

        // 无效银行卡
        assert!(SensitiveMasker::mask("1234567", MaskType::BankCard).is_err());
    }
}
