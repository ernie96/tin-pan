//! Where the saved sign-in (Suno's cookies) is kept, never in plain text. On Windows it's encrypted
//! with DPAPI for the Windows account, the same way and with the same entropy as Tin Pan 1, so 2.0
//! finds a 1.x sign-in and nobody has to sign in again. Elsewhere it goes in the system keychain
//! (macOS Keychain, the Secret Service on Linux) and the settings only note that it's there.

use crate::config::UserConfig;

/// What a saved sign-in in the keychain looks like in the settings.
#[cfg(not(windows))]
const IN_KEYCHAIN: &str = "keychain";

/// The saved cookies, if there are any and they can be read.
pub fn load_sign_in(user: &UserConfig) -> Option<String> {
    if user.protected_cookies.is_empty() {
        return None;
    }
    let found = platform::load(&user.protected_cookies);
    if found.is_none() {
        log::warn!("Saved sign-in couldn't be read; a new sign-in is needed");
    }
    found
}

/// Saves the cookies, or forgets them with None.
pub fn save_sign_in(user: &mut UserConfig, cookies: Option<&str>) {
    user.protected_cookies = match cookies.filter(|c| !c.is_empty()) {
        Some(c) => platform::save(c).unwrap_or_else(|e| {
            log::error!("Couldn't save the sign-in: {e}");
            String::new()
        }),
        None => {
            platform::forget();
            String::new()
        }
    };
}

#[cfg(windows)]
mod platform {
    use base64::Engine;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData};

    const ENTROPY: &[u8] = b"SunoCompanion.cookies.v1";

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr().cast_mut() }
    }

    fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        // SAFETY: DPAPI allocated cbData bytes at pbData with LocalAlloc; they're copied, then freed.
        unsafe {
            let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            LocalFree(out.pbData.cast());
            bytes
        }
    }

    pub fn load(saved: &str) -> Option<String> {
        let encrypted = base64::engine::general_purpose::STANDARD.decode(saved).ok()?;
        let (input, entropy) = (blob(&encrypted), blob(ENTROPY));
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: the blobs point at live buffers for the length given; DPAPI only reads them.
        let ok = unsafe { CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null(), std::ptr::null(), 0, &mut out) };
        if ok == 0 {
            return None;
        }
        String::from_utf8(take(out)).ok()
    }

    pub fn save(cookies: &str) -> Result<String, String> {
        let (input, entropy) = (blob(cookies.as_bytes()), blob(ENTROPY));
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: as in load.
        let ok = unsafe { CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null(), std::ptr::null(), 0, &mut out) };
        if ok == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(base64::engine::general_purpose::STANDARD.encode(take(out)))
    }

    pub fn forget() {}
}

#[cfg(not(windows))]
mod platform {
    use super::IN_KEYCHAIN;

    fn entry() -> keyring::Result<keyring::Entry> {
        keyring::Entry::new("Tin Pan", "Suno sign-in")
    }

    pub fn load(saved: &str) -> Option<String> {
        if saved != IN_KEYCHAIN {
            return None;
        }
        entry().and_then(|e| e.get_password()).ok()
    }

    pub fn save(cookies: &str) -> Result<String, String> {
        entry().and_then(|e| e.set_password(cookies)).map_err(|e| e.to_string())?;
        Ok(IN_KEYCHAIN.to_string())
    }

    pub fn forget() {
        if let Ok(e) = entry() {
            let _ = e.delete_credential();
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn a_saved_sign_in_reads_back_and_forgetting_clears_it() {
        let mut user = UserConfig::default();
        save_sign_in(&mut user, Some("__client=abc; ajs_anonymous_id=d"));
        assert!(!user.protected_cookies.contains("__client"), "never in plain text");
        assert_eq!(load_sign_in(&user).as_deref(), Some("__client=abc; ajs_anonymous_id=d"));
        save_sign_in(&mut user, None);
        assert_eq!(load_sign_in(&user), None);
    }
}
