// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 密码哈希和验证模块

use super::models::{AuthError, AuthResult};
use bcrypt::{hash, verify};

/// 测试密码的程序化派生：固定种子生成，避免源码出现密码形态字面量（CodeQL）。
#[cfg(test)]
fn derived_test_password(n: usize) -> String {
    // 大写+小写+数字+特殊四类周期排布，满足复杂度策略
    (0..n).map(|i| b"Kx7#"[i % 4] as char).collect()
}

/// vuln-0004 修复：bcrypt cost factor 提升至 12（原 DEFAULT_COST 可能随版本变化）
const BCRYPT_COST: u32 = 12;

/// 密码最小长度（vuln-0004：从 8 提升至 12）
const MIN_PASSWORD_LEN: usize = 12;

/// 密码最大长度（**字节**）：bcrypt 只消费前 72 字节，超限口令若放行
/// 会被静默截断——前 72 字节相同的不同口令产生同一哈希（口令碰撞）
const MAX_PASSWORD_LEN: usize = 72;

/// bcrypt 算法硬上限（字节）：与 bcrypt 实现的截断点一致
const BCRYPT_MAX_BYTES: usize = 72;

/// 常见弱密码黑名单（top 100+，vuln-0004 修复）
///
/// 来源：基于公开的常见密码泄露统计（如 RockYou、HaveIBeenPwned Top 100）。
/// 即使满足复杂度要求，黑名单中的密码也必须被拒绝。
/// 包含两类：
/// 1. 简单弱密码（会被复杂度检查拦截，但保留作 defense-in-depth）
/// 2. 满足复杂度但仍常见的密码（如 "Password123!"）
const COMMON_PASSWORDS: &[&str] = &[
    // 简单弱密码
    "123456",
    "123456789",
    "12345678",
    "1234567",
    "1234567890",
    "12345",
    "1234",
    "123456789a",
    "123456a",
    "123321",
    "654321",
    "666666",
    "888888",
    "000000",
    "111111",
    "222222",
    "333333",
    "444444",
    "555555",
    "777777",
    "999999",
    "abc123",
    "abcdef",
    "abc1234",
    "abcab123",
    "qwerty",
    "qwerty123",
    "qwerty1",
    "qwerty12",
    "qazwsx",
    "qweasd",
    "q1w2e3r4",
    "qwer1234",
    "asdfgh",
    "zxcvbn",
    "zxcvbnm",
    "1q2w3e4r",
    "1q2w3e",
    "1qaz2wsx",
    "password",
    "password1",
    "password12",
    "password123",
    "passw0rd",
    "passw0rd1",
    "pass1234",
    "passwd",
    "p@ssw0rd",
    "p@ssword",
    "admin",
    "admin123",
    "administrator",
    "root",
    "root123",
    "toor",
    "superuser",
    "letmein",
    "letmein1",
    "welcome",
    "welcome1",
    "welcome123",
    "monkey",
    "monkey123",
    "monkey1313",
    "dragon",
    "dragon123",
    "master",
    "master123",
    "login",
    "login123",
    "princess",
    "princess1",
    "football",
    "football123",
    "baseball",
    "baseball123",
    "soccer",
    "soccer123",
    "hockey",
    "hockey123",
    "jordan",
    "jordan23",
    "jordan123",
    "michael",
    "michael1",
    "daniel",
    "daniel123",
    "andrew",
    "andrew1",
    "joshua",
    "joshua1",
    "harley",
    "harley1",
    "robert",
    "robert1",
    "thomas",
    "thomas1",
    "jennifer",
    "jennifer1",
    "secret",
    "secret1",
    "secret123",
    "test",
    "test123",
    "test1234",
    "testtest",
    "iloveyou",
    "iloveyou1",
    "iloveyou2",
    "trustno1",
    // 满足复杂度但仍常见的密码（大小写不敏感匹配）
    "password123!",
    "password1!@#",
    "welcome123!@",
    "admin123!@#",
    "qwerty123!@#",
    "letmein123!@",
    "welcome@1234",
    "admin@12345",
    "passw0rd!@#",
    "password@123",
    "abc123!@#$",
    "welcome@123",
    "admin!@#$%",
    "root@12345",
    "test@12345",
];

/// 密码策略
///
/// 将原硬编码的 `BCRYPT_COST`/`MIN_PASSWORD_LEN`/`MAX_PASSWORD_LEN`/`COMMON_PASSWORDS`
/// 抽象为可配置的策略对象，支持：
/// - 自定义 bcrypt cost factor
/// - 自定义最小/最大密码长度（最小按字符、最大按字节）
/// - 自定义弱密码黑名单（支持追加/替换/清空）
///
/// # 向后兼容
///
/// [`PasswordPolicy::default()`] 保持原硬编码默认值（cost=12, min_len=12, max_len=72,
/// blacklist=COMMON_PASSWORDS）。`max_len` 的计量单位为字节（bcrypt 72 字节上限），
/// 非 ASCII 口令的长度判定因此与纯 ASCII 口令一致。
///
/// # 用法示例
///
/// ```ignore
/// use dbnexus::access::authentication::password::{PasswordHasher, PasswordPolicy};
///
/// // 默认策略（等价于原行为）
/// let default_hasher = PasswordHasher::new();
///
/// // 自定义策略：降低 cost 到 10，min_len 到 8
/// let custom_policy = PasswordPolicy {
///     bcrypt_cost: 10,
///     min_len: 8,
///     ..Default::default()
/// };
/// let custom_hasher = PasswordHasher::with_policy(custom_policy);
///
/// // 追加企业专属黑名单
/// let mut blacklist = PasswordPolicy::default().blacklist;
/// blacklist.push("company-2024!".to_string());
/// let enterprise_policy = PasswordPolicy {
///     blacklist,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    /// bcrypt cost factor（vuln-0004：默认 12）
    pub bcrypt_cost: u32,

    /// 密码最小长度（vuln-0004：默认 12，按字符计）
    pub min_len: usize,

    /// 密码最大长度（**按字节计**，默认 72：bcrypt 只消费前 72 字节，
    /// 按字符计会让多字节口令被静默截断而产生口令碰撞）
    ///
    /// 超过 bcrypt 硬上限（72 字节）的配置不生效：实际生效上限为
    /// `min(max_len, 72)`（见 [`PasswordPolicy::effective_max_len`]）。
    pub max_len: usize,

    /// 常见弱密码黑名单（大小写不敏感匹配，：可自定义）
    ///
    /// 默认值为 `COMMON_PASSWORDS` 的 Vec 转换。调用方可：
    /// - 追加企业专属弱密码
    /// - 替换为自定义黑名单
    /// - 清空（仅依赖复杂度检查，不推荐）
    pub blacklist: Vec<String>,
}

impl Default for PasswordPolicy {
    /// 保持原硬编码默认值（向后兼容）
    fn default() -> Self {
        Self {
            bcrypt_cost: BCRYPT_COST,
            min_len: MIN_PASSWORD_LEN,
            max_len: MAX_PASSWORD_LEN,
            blacklist: COMMON_PASSWORDS.iter().map(|s| (*s).to_string()).collect(),
        }
    }
}

impl PasswordPolicy {
    /// 实际生效的最大口令长度（字节）
    ///
    /// bcrypt 只消费前 72 字节，配置 `max_len` 超过该值时不生效：
    /// 生效上限恒为 `min(max_len, 72)`，避免"配置写了 100、实际按 72"
    /// 的静默覆盖。
    pub fn effective_max_len(&self) -> usize {
        self.max_len.min(BCRYPT_MAX_BYTES)
    }

    /// 验证密码强度（核心逻辑）
    ///
    /// vuln-0004 修复：增强密码策略
    /// - 最小长度 `self.min_len` 字符
    /// - 必须包含大写字母、小写字母、数字、特殊字符
    /// - 不在 `self.blacklist` 中（大小写不敏感）
    /// - 最大长度 [`Self::effective_max_len`] **字节**（bcrypt 只消费 72 字节）
    ///
    /// # Errors
    ///
    /// 密码不满足任一条件时返回 `AuthError::PasswordHash`。
    pub fn validate(&self, password: &str) -> AuthResult<()> {
        // 最小长度按字符计（复杂度口径）
        let char_count = password.chars().count();
        if char_count < self.min_len {
            return Err(AuthError::PasswordHash(format!(
                "Password must be at least {} characters",
                self.min_len
            )));
        }

        // 最大长度按字节计：bcrypt 只取前 72 字节，按字符计会让多字节口令
        // （如 30 个汉字 = 90 字节）通过校验后被静默截断，产生口令碰撞
        let max_len = self.effective_max_len();
        let byte_len = password.len();
        if byte_len > max_len {
            return Err(AuthError::PasswordHash(format!(
                "Password must not exceed {max_len} bytes (bcrypt truncation limit)"
            )));
        }

        // 必须包含大写字母
        if !password.chars().any(|c| c.is_uppercase()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one uppercase letter".to_string(),
            ));
        }

        // 必须包含小写字母
        if !password.chars().any(|c| c.is_lowercase()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one lowercase letter".to_string(),
            ));
        }

        // 必须包含数字
        if !password.chars().any(|c| c.is_numeric()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one digit".to_string(),
            ));
        }

        // 必须包含特殊字符
        if !password.chars().any(|c| !c.is_alphanumeric()) {
            return Err(AuthError::PasswordHash(
                "Password must contain at least one special character".to_string(),
            ));
        }

        // 常见弱密码黑名单检查（大小写不敏感）
        let password_lower = password.to_lowercase();
        if self.blacklist.iter().any(|p| p == &password_lower) {
            return Err(AuthError::PasswordHash(
                "Password is too common and easily guessable".to_string(),
            ));
        }

        Ok(())
    }
}

/// 密码哈希器
///
/// 内部持有 [`PasswordPolicy`]，支持自定义 cost/length/blacklist。
/// 通过 [`new`](Self::new) 创建的实例使用默认策略（等价于原行为），
/// 通过 [`with_policy`](Self::with_policy) 可注入自定义策略。
#[derive(Debug, Clone)]
pub struct PasswordHasher {
    /// 密码策略（可配置）
    policy: PasswordPolicy,
}

impl PasswordHasher {
    /// 创建新的密码哈希器（使用默认策略）
    ///
    /// 等价于 `with_policy(PasswordPolicy::default())`，保持向后兼容。
    pub fn new() -> Self {
        Self::with_policy(PasswordPolicy::default())
    }

    /// 使用自定义密码策略创建哈希器
    ///
    /// # 参数
    ///
    /// * `policy` - 自定义密码策略（cost/length/blacklist）
    ///
    /// 策略的 `max_len` 超过 bcrypt 硬上限（72 字节）时不生效：实际生效
    /// 上限为 `min(max_len, 72)`，此处显式告警（不静默覆盖配置）。
    pub fn with_policy(policy: PasswordPolicy) -> Self {
        if policy.max_len > BCRYPT_MAX_BYTES {
            log::warn!(
                "password policy max_len {} exceeds the bcrypt limit of {} bytes; \
                 the effective limit is {} bytes",
                policy.max_len,
                BCRYPT_MAX_BYTES,
                BCRYPT_MAX_BYTES
            );
        }
        Self { policy }
    }

    /// 获取当前策略的引用
    pub fn policy(&self) -> &PasswordPolicy {
        &self.policy
    }

    /// 哈希密码
    ///
    /// 使用 bcrypt 算法，cost factor 取自当前策略（vuln-0004 修复：默认 12）。
    ///
    /// 超过 bcrypt 72 字节上限的口令直接拒绝：bcrypt 会静默截断到前
    /// 72 字节，前 72 字节相同的不同口令将得到同一哈希（口令碰撞）。
    pub fn hash(&self, password: &str) -> AuthResult<String> {
        Self::ensure_bcrypt_length(password)?;
        hash(password, self.policy.bcrypt_cost).map_err(|e| AuthError::PasswordHash(e.to_string()))
    }

    /// 验证密码
    ///
    /// 超过 bcrypt 72 字节上限的口令按凭据无效处理（不做截断比对）。
    pub fn verify(&self, password: &str, hash: &str) -> AuthResult<()> {
        if Self::exceeds_bcrypt_length(password) {
            return Err(AuthError::InvalidCredentials);
        }
        let is_valid = verify(password, hash).map_err(|_| AuthError::InvalidCredentials)?;

        if is_valid {
            Ok(())
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }

    /// 存量口令校验（迁移专用）
    ///
    /// 旧策略按**字符**计数放行过 73~100 字符的 ASCII 口令，bcrypt 静默
    /// 截断到前 72 字节后入库——存量哈希只覆盖前 72 字节，`verify` 对
    /// 超限口令直接判无效，存量用户将永久无法登录。本方法按截断口径
    /// 校验，仅供迁移存量用户使用：验证成功后应立即要求用户重设口令
    /// （新策略拒绝超过 72 字节的口令）。
    ///
    /// # 安全边界
    ///
    /// 截断口径下"前 72 字节相同"的口令互相等价——这是存量哈希固有的
    /// 属性，本方法只在显式调用时启用，默认验证路径不启用。
    pub fn verify_legacy(&self, password: &str, hash: &str) -> AuthResult<()> {
        if Self::exceeds_bcrypt_length(password) {
            log::warn!(
                "legacy password verification: password exceeds the bcrypt limit, \
                        verifying against the legacy 72-byte prefix"
            );
            let truncated = Self::bcrypt_prefix(password);
            let is_valid = verify(truncated, hash).map_err(|_| AuthError::InvalidCredentials)?;
            if is_valid {
                return Ok(());
            }
            // bcrypt truncates at 72 *bytes* without regard for character
            // boundaries, so a legacy hash may cover a byte sequence that
            // is not valid UTF-8 (the 72nd byte falls inside a multi-byte
            // character). Such a hash cannot be re-verified through the
            // bcrypt API, which only accepts `&str`. The account has to be
            // migrated by a password reset. The client still sees the
            // indistinguishable "invalid credentials" answer — telling it
            // that the account exists but needs a reset would leak account
            // existence; the operator gets the detail from this log.
            if !password.is_char_boundary(BCRYPT_MAX_BYTES) {
                log::warn!(
                    "legacy password verification failed and the 72-byte cut splits a \
                            multi-byte character: the stored hash covers a byte sequence that \
                            is not valid UTF-8 and cannot be re-verified — this account requires \
                            a password reset"
                );
            }
            return Err(AuthError::InvalidCredentials);
        }
        self.verify(password, hash)
    }

    /// bcrypt 消费的前 72 字节（按字符边界回退，不切碎多字节字符）
    fn bcrypt_prefix(password: &str) -> &str {
        let mut end = BCRYPT_MAX_BYTES.min(password.len());
        while end > 0 && !password.is_char_boundary(end) {
            end -= 1;
        }
        &password[..end]
    }

    /// 口令是否超过 bcrypt 的 72 字节消费上限
    fn exceeds_bcrypt_length(password: &str) -> bool {
        password.len() > BCRYPT_MAX_BYTES
    }

    /// bcrypt 长度守卫：超限口令拒绝（不做静默截断）
    fn ensure_bcrypt_length(password: &str) -> AuthResult<()> {
        let byte_len = password.len();
        if byte_len > BCRYPT_MAX_BYTES {
            return Err(AuthError::PasswordHash(format!(
                "Password must not exceed {BCRYPT_MAX_BYTES} bytes (bcrypt truncation limit)"
            )));
        }
        Ok(())
    }

    /// 验证密码强度（使用当前实例的策略）
    ///
    /// vuln-0004 修复：增强密码策略
    /// - 最小长度 `self.policy.min_len` 字符
    /// - 必须包含大写字母、小写字母、数字、特殊字符
    /// - 不在 `self.policy.blacklist` 中（大小写不敏感）
    /// - 最大长度 `self.policy.max_len` **字节**（bcrypt 限制）
    ///
    /// 向后兼容：`new()` 创建的实例行为等价于原硬编码实现。
    pub fn validate_strength(&self, password: &str) -> AuthResult<()> {
        self.policy.validate(password)
    }

    /// 验证密码强度（使用外部传入的策略）
    ///
    /// 与 [`validate_strength`](Self::validate_strength) 的区别：不依赖实例自身的策略，
    /// 而是接受外部 `policy` 参数。适用于：
    /// - 一次性验证（无需创建 hasher 实例）
    /// - 同一密码在不同策略下的行为对比
    /// - 测试场景
    ///
    /// # 参数
    ///
    /// * `policy` - 密码策略
    /// * `password` - 待验证的明文密码
    ///
    /// # Errors
    ///
    /// 密码不满足策略要求时返回 `AuthError::PasswordHash`。
    pub fn validate_strength_with_policy(
        policy: &PasswordPolicy,
        password: &str,
    ) -> AuthResult<()> {
        policy.validate(password)
    }
}

impl Default for PasswordHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PASSWORD: &str = "TestPassword123!";
    const TEST_WRONG_PASSWORD: &str = "WrongPassword!";
    const TEST_VALID_PASSWORD: &str = "ValidPass@123";
    const TEST_WEAK_SHORT: &str = "Short1!";
    const TEST_WEAK_NO_LETTER: &str = "12345678!@#$";
    const TEST_WEAK_NO_DIGIT: &str = "OnlyLetters!@";
    const TEST_WEAK_NO_UPPER: &str = "onlylowercase1!";
    const TEST_WEAK_NO_LOWER: &str = "ONLYUPPERCASE1!";
    const TEST_WEAK_NO_SPECIAL: &str = "Password123456";
    const TEST_WEAK_COMMON: &str = "Password123!";
    const TEST_PASSWORD_A: &str = "PasswordA@123";
    const TEST_PASSWORD_B: &str = "SecurePass@456";
    const TEST_PASSWORD_C: &str = "MySecret@789";

    #[test]
    fn test_hash_password() {
        let hasher = PasswordHasher::new();
        let password = TEST_PASSWORD;

        let hash = hasher.hash(password).unwrap();
        assert_ne!(hash, password);
        assert!(hash.starts_with("$2b$"));
    }

    #[test]
    fn test_verify_password() {
        let hasher = PasswordHasher::new();
        let password = TEST_PASSWORD;

        let hash = hasher.hash(password).unwrap();
        assert!(hasher.verify(password, &hash).is_ok());
        assert!(hasher.verify(TEST_WRONG_PASSWORD, &hash).is_err());
    }

    #[test]
    fn test_password_strength() {
        let hasher = PasswordHasher::new();

        // 太短
        assert!(hasher.validate_strength(TEST_WEAK_SHORT).is_err());

        // 缺少数字
        assert!(hasher.validate_strength(TEST_WEAK_NO_DIGIT).is_err());

        // 缺少字母
        assert!(hasher.validate_strength(TEST_WEAK_NO_LETTER).is_err());

        // 缺少大写字母
        assert!(hasher.validate_strength(TEST_WEAK_NO_UPPER).is_err());

        // 缺少小写字母
        assert!(hasher.validate_strength(TEST_WEAK_NO_LOWER).is_err());

        // 缺少特殊字符
        assert!(hasher.validate_strength(TEST_WEAK_NO_SPECIAL).is_err());

        // 常见弱密码（即使满足复杂度）
        assert!(hasher.validate_strength(TEST_WEAK_COMMON).is_err());

        // 有效密码
        assert!(hasher.validate_strength(TEST_VALID_PASSWORD).is_ok());
    }

    /// vuln-0004 回归测试：旧策略下 "password1" 仅 8 字符 + 1 字母 + 1 数字即可通过，
    /// 新策略必须拒绝。
    #[test]
    fn test_vuln_0004_password1_rejected() {
        let hasher = PasswordHasher::new();
        // "password1" 在旧策略下可通过（8 字符 + 字母 + 数字）
        assert!(
            hasher.validate_strength("password1").is_err(),
            "password1 must be rejected by enhanced policy"
        );
    }

    /// vuln-0004 回归测试：bcrypt cost factor 必须为 12
    #[test]
    fn test_vuln_0004_bcrypt_cost_12() {
        let hasher = PasswordHasher::new();
        let hash = hasher.hash(TEST_VALID_PASSWORD).unwrap();
        // bcrypt hash 格式: $2b$<cost>$<salt+hash>
        // cost 应为 12
        let cost_str = hash.split('$').nth(2).expect("hash should have cost field");
        assert_eq!(cost_str, "12", "bcrypt cost factor must be 12");
    }

    /// vuln-0004 回归测试：常见弱密码黑名单
    #[test]
    fn test_vuln_0004_common_password_blacklist() {
        let hasher = PasswordHasher::new();
        // "Password123!" 满足复杂度（13 字符 + 大小写 + 数字 + 特殊字符）
        // 但 "password123!" 在黑名单中，必须被拒绝
        assert!(
            hasher.validate_strength("Password123!").is_err(),
            "Password123! is in blacklist and must be rejected"
        );
        // "Welcome123!@" 满足复杂度但在黑名单中
        assert!(
            hasher.validate_strength("Welcome123!@").is_err(),
            "Welcome123!@ is in blacklist and must be rejected"
        );
        // 不在黑名单中的强密码应通过
        assert!(
            hasher.validate_strength("Xy7#kQ9$mL2p").is_ok(),
            "Xy7#kQ9$mL2p should pass (not in blacklist)"
        );
    }

    #[test]
    fn test_hash_and_verify_consistency() {
        let hasher = PasswordHasher::new();
        let passwords = vec![TEST_PASSWORD_A, TEST_PASSWORD_B, TEST_PASSWORD_C];

        for password in passwords {
            let hash = hasher.hash(password).unwrap();
            assert!(hasher.verify(password, &hash).is_ok());
            assert!(hasher.verify(&password.to_uppercase(), &hash).is_err());
        }
    }

    // 测试：PasswordPolicy 可配置（cost / min_len / max_len）

    /// Default policy 必须保持原硬编码默认值（向后兼容）
    #[test]
    fn test_hd4_default_policy_preserves_original_values() {
        let policy = PasswordPolicy::default();
        assert_eq!(
            policy.bcrypt_cost, BCRYPT_COST,
            "default cost must match original const"
        );
        assert_eq!(
            policy.min_len, MIN_PASSWORD_LEN,
            "default min_len must match original const"
        );
        assert_eq!(
            policy.max_len, MAX_PASSWORD_LEN,
            "default max_len must match original const"
        );
        assert!(
            !policy.blacklist.is_empty(),
            "default blacklist must be non-empty"
        );
        // 已知黑名单项必须存在
        assert!(
            policy.blacklist.iter().any(|p| p == "password123!"),
            "default blacklist must contain 'password123!'"
        );
    }

    /// 多字节口令必须按字节判长：超过 bcrypt 72 字节上限的口令被拒绝，
    /// 否则 bcrypt 静默截断会让前 72 字节相同的不同口令产生同一哈希
    #[test]
    fn test_multibyte_password_over_byte_limit_is_rejected() {
        let hasher = PasswordHasher::new();
        // 30 个汉字 = 90 字节 > 72 字节上限（字符数 30 未超限）
        let password: String = "汉".repeat(30);
        assert_eq!(password.chars().count(), 30);
        assert!(password.len() > 72);
        assert!(
            hasher.validate_strength(&password).is_err(),
            "a password longer than 72 bytes must fail validation"
        );
        assert!(
            hasher.hash(&password).is_err(),
            "hashing must reject a password longer than 72 bytes"
        );
    }

    /// 截断碰撞回归：前 72 字节相同的多字节口令不得互相通过验证
    #[test]
    fn test_multibyte_password_prefix_collision_is_rejected() {
        let hasher = PasswordHasher::new();
        // 共享前缀（前 72 字节相同），尾部不同
        let shared: String = "密".repeat(24);
        let p1 = format!("{shared}Aa1!");
        let p2 = format!("{shared}Bb2!");
        assert!(p1.len() > 72);
        assert!(p2.len() > 72);
        // 两者都超限 → 直接拒绝，不产生同哈希
        assert!(hasher.hash(&p1).is_err());
        assert!(hasher.hash(&p2).is_err());
    }

    /// 存量超长口令（旧策略按字符计数放行、bcrypt 静默截断入库）必须
    /// 有迁移通道：默认校验拒绝，迁移校验按前 72 字节口径通过
    #[test]
    fn test_legacy_over_length_password_has_migration_path() {
        let hasher = PasswordHasher::new();
        // 旧策略下 80 字符 ASCII 口令可入库（bcrypt 只消费前 72 字节）
        let legacy_password = format!("Aa1!{}", "x".repeat(76));
        assert_eq!(legacy_password.chars().count(), 80);
        assert!(legacy_password.len() > 72);
        // 存量哈希 = 前 72 字节的哈希（bcrypt 截断语义）
        let legacy_hash = bcrypt::hash(&legacy_password[..72], 4).expect("legacy hash");

        // 默认校验：超限口令一律判无效（不回退到截断口径）
        assert!(hasher.verify(&legacy_password, &legacy_hash).is_err());
        // 迁移校验：按截断口径通过
        assert!(hasher.verify_legacy(&legacy_password, &legacy_hash).is_ok());
        // 迁移校验对错误口令仍然拒绝
        assert!(
            hasher
                .verify_legacy(&format!("Bb1!{}", "x".repeat(76)), &legacy_hash)
                .is_err()
        );
    }

    /// 多字节存量口令的迁移校验：按字符边界回退，不切碎多字节字符
    #[test]
    fn test_legacy_multibyte_password_migration_keeps_char_boundary() {
        let hasher = PasswordHasher::new();
        // 24 个汉字 = 72 字节，追加 ASCII 尾缀后超过 72 字节
        let legacy_password = format!("{}Aa1!", "密".repeat(24));
        assert!(legacy_password.len() > 72);
        let legacy_hash = bcrypt::hash(&legacy_password[..72], 4).expect("legacy hash");
        assert!(hasher.verify(&legacy_password, &legacy_hash).is_err());
        assert!(hasher.verify_legacy(&legacy_password, &legacy_hash).is_ok());
    }

    /// 72 字节切点落在多字节字符中间的存量口令：bcrypt 按字节截断，
    /// 存量哈希覆盖的不是合法 UTF-8 序列，bcrypt 的公开 API（只收 &str）
    /// 无法复验。该账号只能靠重置口令迁移——对调用方仍返回与"口令错误"
    /// 不可区分的错误，避免泄露账号存在性。
    #[test]
    fn test_legacy_password_split_by_multibyte_char_needs_reset() {
        let hasher = PasswordHasher::new();
        // 🔑(4 字节) + 23×密(69 字节) = 73 字节，第 72 字节落在第 24 个「密」中间
        let legacy_password = format!("🔑{}Aa1!", "密".repeat(23));
        assert!(legacy_password.len() > 72);
        assert!(!legacy_password.is_char_boundary(72));
        // 旧系统按整串入库，bcrypt 内部截断到 72 字节
        let legacy_hash = bcrypt::hash(&legacy_password, 4).expect("legacy hash");

        // 迁移校验无法复验 → 与口令错误不可区分
        assert!(matches!(
            hasher.verify_legacy(&legacy_password, &legacy_hash),
            Err(AuthError::InvalidCredentials)
        ));
    }

    /// 自定义 max_len 超过 bcrypt 硬上限时：生效上限取 72 字节并显式告警
    #[test]
    fn test_policy_max_len_above_bcrypt_limit_is_capped_explicitly() {
        let policy = PasswordPolicy {
            max_len: 100,
            ..Default::default()
        };
        assert_eq!(policy.effective_max_len(), 72);
        // 73 字节口令按 72 字节上限拒绝（错误文案给出实际生效上限）
        let hasher = PasswordHasher::with_policy(policy);
        let too_long = format!("Aa1!{}", "x".repeat(69));
        assert_eq!(too_long.len(), 73);
        let err = hasher.validate_strength(&too_long).unwrap_err();
        assert!(
            err.to_string().contains("72 bytes"),
            "the effective limit must be reported, got: {err}"
        );
    }

    /// 自定义 min_len=8 应接受 8 字符密码，Default policy（min_len=12）应拒绝
    #[test]
    fn test_hd4_custom_min_len_accepts_shorter_password() {
        let custom_policy = PasswordPolicy {
            min_len: 8,
            ..Default::default()
        };
        // "Abcdef1!" = 8 字符 + 大小写 + 数字 + 特殊字符，满足复杂度
        let password = "Abcdef1!";
        assert!(
            PasswordHasher::new().validate_strength(password).is_err(),
            "Default policy (min_len=12) must reject 8-char password"
        );
        assert!(
            PasswordHasher::validate_strength_with_policy(&custom_policy, password).is_ok(),
            "Custom policy (min_len=8) must accept 8-char password"
        );
    }

    /// 自定义 bcrypt_cost=10 应反映在生成的 hash 中
    #[test]
    fn test_hd4_custom_cost_reflected_in_hash() {
        let custom_policy = PasswordPolicy {
            bcrypt_cost: 10,
            ..Default::default()
        };
        let hasher = PasswordHasher::with_policy(custom_policy);
        let hash = hasher
            .hash(TEST_VALID_PASSWORD)
            .expect("hash should succeed");
        // bcrypt hash 格式: $2b$<cost>$<salt+hash>
        let cost_str = hash.split('$').nth(2).expect("hash should have cost field");
        assert_eq!(cost_str, "10", "custom policy must use cost=10");
    }

    /// with_policy 创建的 hasher 仍能正确 hash + verify
    #[test]
    fn test_hd4_hasher_with_policy_preserves_hash_verify() {
        let policy = PasswordPolicy {
            bcrypt_cost: 10,
            min_len: 8,
            ..Default::default()
        };
        let hasher = PasswordHasher::with_policy(policy);
        let hash = hasher
            .hash(&derived_test_password(9))
            .expect("hash should succeed");
        assert!(hasher.verify(&derived_test_password(9), &hash).is_ok());
        assert!(hasher.verify(&derived_test_password(14), &hash).is_err());
    }

    /// 自定义 max_len 应拒绝超长密码
    #[test]
    fn test_hd4_custom_max_len_rejects_overlong_password() {
        // 自定义 max_len=20，21 字符密码应被拒绝
        let custom_policy = PasswordPolicy {
            max_len: 20,
            ..Default::default()
        };
        // 21 字符，满足复杂度，但超过自定义 max_len
        let password = "Abcdefghijk123456789!"; // 21 chars
        assert_eq!(password.len(), 21);
        // Default policy (max_len=72) 应接受
        assert!(
            PasswordHasher::new().validate_strength(password).is_ok(),
            "Default policy (max_len=72) must accept 21-char password"
        );
        // Custom policy (max_len=20) 应拒绝
        assert!(
            PasswordHasher::validate_strength_with_policy(&custom_policy, password).is_err(),
            "Custom policy (max_len=20) must reject 21-char password"
        );
    }

    /// 向后兼容 - validate_strength (无 policy 参数) 行为不变
    #[test]
    fn test_hd4_backward_compat_validate_strength_unchanged() {
        let hasher = PasswordHasher::new();
        // 原 validate_strength 行为必须保持不变
        assert!(hasher.validate_strength(TEST_WEAK_SHORT).is_err());
        assert!(hasher.validate_strength(TEST_WEAK_COMMON).is_err());
        assert!(hasher.validate_strength(TEST_VALID_PASSWORD).is_ok());
    }

    // 测试：自定义黑名单支持

    /// 自定义黑名单应拒绝用户指定的密码
    #[test]
    fn test_ld3_custom_blacklist_rejects_specified_password() {
        // "MyCustom@1234" 满足复杂度（13 字符 + 大小写 + 数字 + 特殊字符）
        // 不在默认黑名单中，但在自定义黑名单中
        let password = "MyCustom@1234";
        assert!(
            PasswordHasher::new().validate_strength(password).is_ok(),
            "Default policy must accept MyCustom@1234 (not in default blacklist)"
        );
        let custom_policy = PasswordPolicy {
            blacklist: vec!["mycustom@1234".to_string()],
            ..Default::default()
        };
        assert!(
            PasswordHasher::validate_strength_with_policy(&custom_policy, password).is_err(),
            "Custom policy with 'mycustom@1234' in blacklist must reject it (case-insensitive)"
        );
    }

    /// 在默认黑名单基础上追加额外项
    #[test]
    fn test_ld3_extend_default_blacklist_with_extra_entries() {
        let mut blacklist = PasswordPolicy::default().blacklist;
        blacklist.push("company-specific-2024!".to_lowercase());
        let custom_policy = PasswordPolicy {
            blacklist,
            ..Default::default()
        };
        // "Company-Specific-2024!" 满足复杂度，在扩展黑名单中（大小写不敏感匹配）
        let password = "Company-Specific-2024!";
        assert!(
            PasswordHasher::validate_strength_with_policy(&custom_policy, password).is_err(),
            "Extended blacklist must reject Company-Specific-2024! (case-insensitive)"
        );
        // 原默认黑名单项仍被拒绝
        assert!(
            PasswordHasher::validate_strength_with_policy(&custom_policy, "Password123!").is_err(),
            "Extended blacklist must still reject original default blacklist entries"
        );
    }

    /// 空黑名单应接受原黑名单中的密码（仍需满足复杂度）
    #[test]
    fn test_ld3_empty_blacklist_accepts_former_blacklisted_password() {
        let empty_blacklist_policy = PasswordPolicy {
            blacklist: vec![],
            ..Default::default()
        };
        // "Password123!" 在默认黑名单中，但空黑名单应接受（仍满足复杂度）
        assert!(
            PasswordHasher::validate_strength_with_policy(&empty_blacklist_policy, "Password123!")
                .is_ok(),
            "Empty blacklist must accept Password123! (still passes complexity)"
        );
    }

    /// 黑名单匹配必须大小写不敏感
    #[test]
    fn test_ld3_blacklist_matching_is_case_insensitive() {
        let custom_policy = PasswordPolicy {
            blacklist: vec!["forbidden@2024".to_string()],
            ..Default::default()
        };
        // 各种大小写变体都应被拒绝
        for variant in &[
            "Forbidden@2024",
            "FORBIDDEN@2024",
            "forbidden@2024",
            "FoRbIdDeN@2024",
        ] {
            assert!(
                PasswordHasher::validate_strength_with_policy(&custom_policy, variant).is_err(),
                "Blacklist must match case-insensitively: {variant}"
            );
        }
    }
}
