// 凭据加密密钥环：current 加密新数据，old 为宽限窗口（仅解密）。
// 轮换文件持久化 env 形态密钥串，重启可还原；避免密钥只活在一个进程的 env 里。
use ecat_security::crypto::{decrypt, derive_key, new_key_b64};
use serde_json::{Value, json};
use std::env;

/// 一把密钥：env 形态字符串（SHA-256 派生前）+ 派生后的 AES 密钥。
#[derive(Clone)]
struct KeySlot {
    src: String,
    key: [u8; 32],
}

/// 密钥环。version 语义：1 = 环境密钥时代；2 = 已轮换（新数据用新密钥）。
/// ponytail: 双密钥窗口只覆盖上一把；连续轮换两次后最早一代数据将无法解密，
/// 届时需先重加密再轮换（有真实场景再说）。
#[derive(Clone)]
pub struct KeyRing {
    current: KeySlot,
    old: Option<KeySlot>,
    version: i64,
}

/// 密钥轮换持久化文件（{"current": "...", "old": "..."}，均为 env 形态字符串）。
fn key_file_path() -> std::path::PathBuf {
    env::var("IOT_CRED_KEYS_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("./cred_keys.json"))
}

impl KeyRing {
    fn slot(src: &str) -> KeySlot {
        KeySlot { src: src.to_string(), key: derive_key(src) }
    }

    /// 启动装配：存在轮换文件（IOT_CRED_KEYS_FILE）则文件优先
    /// （current 为新密钥、old 为上一把），否则 current 取环境密钥、
    /// old 取可选的 IOT_CRED_ENCRYPT_KEY_OLD。
    pub fn load(enc_key_env: &str) -> Self {
        let env_ring = || Self {
            current: Self::slot(enc_key_env),
            old: env::var("IOT_CRED_ENCRYPT_KEY_OLD").ok().map(|s| Self::slot(&s)),
            version: 1,
        };
        match std::fs::read_to_string(key_file_path()) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(v) if v["current"].as_str().is_some() => Self {
                    current: Self::slot(v["current"].as_str().unwrap()),
                    old: env::var("IOT_CRED_ENCRYPT_KEY_OLD")
                        .ok()
                        .map(|s| Self::slot(&s))
                        .or_else(|| v["old"].as_str().map(Self::slot)),
                    version: 2,
                },
                _ => env_ring(),
            },
            Err(_) => env_ring(),
        }
    }

    /// 当前密钥版本（新写入数据的版本号）。
    pub fn version(&self) -> i64 {
        self.version
    }

    /// 当前 AES 密钥（新数据加密用）。
    pub fn current_key(&self) -> &[u8; 32] {
        &self.current.key
    }

    /// 轮换：新随机密钥成为 current（version=2），旧密钥移入宽限窗口，
    /// 持久化到轮换文件（重启后仍可解密旧数据）。返回新密钥 env 形态字符串，
    /// 供运维同步到 IOT_CRED_ENCRYPT_KEY。
    pub fn rotate(&mut self) -> Result<String, String> {
        let new_src = new_key_b64();
        let payload = json!({ "current": new_src, "old": self.current.src });
        let path = key_file_path();
        if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("key file dir: {e}"))?;
        }
        std::fs::write(&path, payload.to_string()).map_err(|e| format!("key file write: {e}"))?;
        // 先写后 chmod：首次创建时路径不存在，先 set_permissions 会静默失败
        // （`let _` 吞错 → 文件按 umask 默认权限落盘，密钥材料组/其他可读）。
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("key file chmod: {e}"))?;
        }
        self.old = Some(std::mem::replace(&mut self.current, Self::slot(&new_src)));
        self.version = 2;
        Ok(new_src)
    }

    /// 按版本选主密钥解密，失败回退另一把（覆盖旧列默认值漂移/误标版本）。
    pub fn decrypt(&self, version: i64, enc: &str) -> Result<Vec<u8>, String> {
        let (primary, alt) = if version >= 2 {
            (&self.current.key, self.old.as_ref().map(|s| &s.key).unwrap_or(&self.current.key))
        } else {
            (self.old.as_ref().map(|s| &s.key).unwrap_or(&self.current.key), &self.current.key)
        };
        match decrypt(primary, enc) {
            Ok(p) => Ok(p),
            Err(_) => decrypt(alt, enc),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecat_security::crypto::encrypt;

    /// 进程级 IOT_CRED_KEYS_FILE 是所有 env 相关测试共享的全局状态：static 锁
    /// 串行化 set_var/写文件，唯一临时目录保证互不覆盖，Drop 时清理
    /// （panic 路径也会执行 Drop，不残留 env/磁盘）。
    static KEY_FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// env/磁盘守卫：持有锁 + 唯一临时目录，Drop 时 remove_var + remove_dir_all。
    struct KeyFileGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        dir: std::path::PathBuf,
    }

    impl KeyFileGuard {
        fn acquire() -> Self {
            let _lock = KEY_FILE_LOCK.lock().unwrap();
            let dir = std::env::temp_dir().join(format!("iot-cred-keys-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            // edition 2024：set_var 为 unsafe（持锁串行，安全）
            unsafe { std::env::set_var("IOT_CRED_KEYS_FILE", dir.join("keys.json")) };
            KeyFileGuard { _lock, dir }
        }
    }

    impl Drop for KeyFileGuard {
        fn drop(&mut self) {
            unsafe { std::env::remove_var("IOT_CRED_KEYS_FILE") };
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    #[test]
    fn decrypt_pre_rotation_uses_env_key() {
        let _guard = KeyFileGuard::acquire();
        // 无轮换：version=1 行由环境密钥加密/解密
        // 直接构造（不经 load，避免读进程级 env/磁盘文件与其他测试串扰）
        let ring = KeyRing { current: KeyRing::slot("env-key"), old: None, version: 1 };
        assert_eq!(ring.version(), 1);
        let enc = encrypt(ring.current_key(), b"secret").unwrap();
        assert_eq!(ring.decrypt(1, &enc).unwrap(), b"secret");
    }

    #[test]
    fn decrypt_after_rotation_old_rows_still_decrypt() {
        let _guard = KeyFileGuard::acquire();
        // 轮换后：version=1 行（旧密钥）由 old 槽解密；version=2 新行由 current 解密。
        // 直接构造 current=新密钥 状态验证逻辑（不经 rotate()：落盘路径归
        // rotate_persists_and_reloads 专测，避免跨测试污染 env/文件路径）
        let old_slot = KeyRing::slot("old-env-key");
        let ring = KeyRing { current: KeyRing::slot("new-key"), old: Some(old_slot.clone()), version: 2 };
        let old_enc = encrypt(&old_slot.key, b"old-data").unwrap();
        let new_enc = encrypt(ring.current_key(), b"new-data").unwrap();
        assert_eq!(ring.decrypt(1, &old_enc).unwrap(), b"old-data");
        assert_eq!(ring.decrypt(2, &new_enc).unwrap(), b"new-data");
        // 版本错标也兜底（回退另一把）
        assert_eq!(ring.decrypt(2, &old_enc).unwrap(), b"old-data");
        assert_eq!(ring.decrypt(1, &new_enc).unwrap(), b"new-data");
        // 新密钥立即生效，上一把仍在宽限窗口
        let fresh_enc = encrypt(ring.current_key(), b"fresh").unwrap();
        assert_eq!(ring.decrypt(2, &fresh_enc).unwrap(), b"fresh");
        assert_eq!(ring.decrypt(2, &new_enc).unwrap(), b"new-data", "上一把仍在宽限窗口");
    }

    #[test]
    fn decrypt_two_rotations_drops_first_generation() {
        // 双密钥窗口只覆盖上一把：连续轮换两次后最早一代（K1）无法解密。
        // 不碰 rotate/load/env，无需 guard。
        let k1 = KeyRing::slot("k1");
        let ring = KeyRing { current: KeyRing::slot("k3"), old: Some(KeyRing::slot("k2")), version: 2 };
        let enc = encrypt(&k1.key, b"ancient").unwrap();
        assert!(ring.decrypt(1, &enc).is_err());
    }

    #[test]
    fn rotate_persists_and_reloads() {
        let _guard = KeyFileGuard::acquire();
        let mut ring = KeyRing::load("env-key");
        assert_eq!(ring.version(), 1);
        let new_src = ring.rotate().unwrap();
        assert_eq!(ring.current.src, new_src);
        assert_eq!(ring.old.as_ref().unwrap().src, "env-key");
        assert_eq!(derive_key(&new_src), ring.current.key);
        // 密钥文件必须 0600（首次创建时 umask 可能放行 0644，rotate 已后置 chmod）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(_guard.dir.join("keys.json")).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "key file must be 0600");
        }
        // 文件持久化，重启（新 KeyRing::load）可还原
        let again = KeyRing::load("unrelated-env");
        assert_eq!(again.version(), 2);
        assert_eq!(again.current.src, new_src);
        assert_eq!(again.old.as_ref().unwrap().src, "env-key");
        // Drop 时清理临时目录 + env
    }
}
