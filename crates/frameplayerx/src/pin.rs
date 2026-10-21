//! PINs that lock a source, so other people using the headset don't see it.
//!
//! A locked source can't be browsed and its videos are left out of the
//! library, history, search and both remotes (see
//! [`fp_library::Library::set_hidden`]); a video of it can't be
//! opened. Entering the PIN unlocks it until FramePlayerX restarts or it is
//! locked again. A lock is the source's own: another source with the same
//! files or server stays open. This keeps guests out, not someone with the
//! headset's files: the videos themselves are not encrypted.
//!
//! Settings keep a salted PBKDF2 hash, never the PIN.

use ring::rand::{SecureRandom, SystemRandom};
use std::collections::{BTreeMap, HashSet};
use std::num::NonZeroU32;

/// Whether source `id` has a PIN and was not unlocked.
pub fn locked(pins: &BTreeMap<String, String>, unlocked: &HashSet<String>, id: &str) -> bool {
    pins.contains_key(id) && !unlocked.contains(id)
}

pub fn locked_ids(pins: &BTreeMap<String, String>, unlocked: &HashSet<String>) -> Vec<String> {
    pins.keys()
        .filter(|id| !unlocked.contains(*id))
        .cloned()
        .collect()
}

/// Digits a PIN may have.
pub const LENGTH: std::ops::RangeInclusive<usize> = 4..=8;

const ITERATIONS: u32 = 100_000;
const PREFIX: &str = "pbkdf2-sha256";

/// Whether `pin` is 4 to 8 digits.
pub fn valid(pin: &str) -> bool {
    LENGTH.contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

/// `pbkdf2-sha256$<iterations>$<salt hex>$<hash hex>` for `pin`.
pub fn hash(pin: &str) -> String {
    let mut salt = [0u8; 16];
    if SystemRandom::new().fill(&mut salt).is_err() {
        // No randomness: a fixed salt still keeps the PIN out of the file.
        log::warn!("no random salt for the PIN");
    }
    let out = derive(pin, &salt, ITERATIONS);
    format!("{PREFIX}${ITERATIONS}${}${}", hex(&salt), hex(&out))
}

/// Whether `pin` matches `stored` (from [`hash`]).
pub fn verify(stored: &str, pin: &str) -> bool {
    let mut parts = stored.split('$');
    let (Some(PREFIX), Some(iterations), Some(salt), Some(expected), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return false;
    };
    let (Some(iterations), Some(salt), Some(expected)) = (
        iterations.parse::<u32>().ok().and_then(NonZeroU32::new),
        unhex(salt),
        unhex(expected),
    ) else {
        return false;
    };
    ring::pbkdf2::verify(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        &salt,
        pin.as_bytes(),
        &expected,
    )
    .is_ok()
}

fn derive(pin: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(iterations).unwrap_or(NonZeroU32::MIN),
        salt,
        pin.as_bytes(),
        &mut out,
    );
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    s.as_bytes()
        .chunks(2)
        .map(|c| {
            let c = std::str::from_utf8(c).ok().filter(|c| c.len() == 2)?;
            u8::from_str_radix(c, 16).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_hash_and_verify() {
        assert!(valid("1234") && valid("12345678"));
        assert!(!valid("123") && !valid("123456789") && !valid("12a4") && !valid(""));
        let h = hash("2468");
        assert!(h.starts_with("pbkdf2-sha256$100000$"), "{h}");
        assert!(!h.contains("2468"));
        assert!(verify(&h, "2468"));
        assert!(!verify(&h, "2469"));
        assert_ne!(h, hash("2468"), "salted");
        assert!(!verify("garbage", "2468"));
        assert!(!verify("pbkdf2-sha256$0$00$00", "2468"));
    }
}
