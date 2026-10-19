//! The mandatory password: length, a zxcvbn-style strength check, and Argon2id.

use anyhow::{Result, bail};
#[cfg(test)]
use std::cell::Cell;
use zeroize::Zeroizing;

/// Argon2id settings stored next to anything they protect (the pin file, and later
/// backup headers). Memory is kibibytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Argon2Params {
    /// Memory cost in KiB. The floor for real use is 64 MiB.
    pub m_kib: u32,
    /// Time cost (iterations).
    pub t: u32,
    /// Parallelism.
    pub p: u32,
}

impl Argon2Params {
    /// Default for a real account: the plan's 64 MiB floor, three iterations, one lane.
    ///
    /// The plan also asks for about one second on the user's machine. This build uses
    /// the floor with `t = 3` rather than a timing loop, so two machines with the same
    /// phrase and password derive the same master without a calibration step. The
    /// parameters are still stored, so a later release can raise them for new accounts.
    pub const PRODUCTION: Self = Self {
        m_kib: 64 * 1024,
        t: 3,
        p: 1,
    };

    /// Tiny parameters for tests. Below the floor on purpose; [`default_params`] never
    /// returns this.
    pub const FAST: Self = Self {
        m_kib: 8,
        t: 1,
        p: 1,
    };

    /// Whether these parameters meet the 64 MiB floor.
    pub fn meets_floor(self) -> bool {
        self.m_kib >= Self::PRODUCTION.m_kib && self.t >= 1 && self.p >= 1
    }
}

/// Parameters a new account uses.
pub fn default_params() -> Argon2Params {
    Argon2Params::PRODUCTION
}

/// Minimum password length, in Unicode scalar values. The strength check is separate.
pub const MIN_PASSWORD_CHARS: usize = 8;

/// Reject a password that is shorter than [`MIN_PASSWORD_CHARS`] or that zxcvbn scores below 3.
pub fn check_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        bail!(
            "the password must be at least {MIN_PASSWORD_CHARS} characters, and there is no password reset"
        );
    }
    let guess = zxcvbn::zxcvbn(password, &[]);
    if guess.score() < zxcvbn::Score::Three {
        let mut msg = String::from("that password is too easy to guess");
        if let Some(w) = guess.feedback().and_then(|f| f.warning()) {
            msg.push_str(": ");
            msg.push_str(&w.to_string());
        }
        bail!("{msg}");
    }
    Ok(())
}

/// Argon2id, 32-byte output. `salt` is the full salt the caller stored
/// (16 bytes for a database wrap, 32 for the master secret).
pub fn argon2id(password: &str, salt: &[u8], params: Argon2Params) -> Result<[u8; 32]> {
    use argon2::{Algorithm, Argon2, Params, Version};
    #[cfg(test)]
    if ARGON_COUNTING.with(Cell::get) {
        ARGON_CALLS.with(|n| n.set(n.get() + 1));
    }
    let p = Params::new(params.m_kib, params.t, params.p, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 parameters: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| anyhow::anyhow!("argon2: {e}"))?;
    Ok(out)
}

/// One Argon2id stretch of a password. Every key this operation needs is an
/// HKDF label of that single output, so a backup, the local database key, and
/// the account key do not each pay for another 64 MiB stretch.
///
/// A second stretch would not make the password harder to guess: an attacker
/// who can read one wrap only has to break one of them. Wraps written earlier,
/// each with its own salt, still open; those salts are stored on the wrap.
pub(crate) struct Stretch {
    salt: [u8; 16],
    params: Argon2Params,
    raw: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for Stretch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Stretch")
    }
}

impl Stretch {
    /// Stretch `password` with a new random salt.
    pub(crate) fn fresh(password: &str, params: Argon2Params) -> Result<Self> {
        let mut salt = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut salt);
        Self::from_salt(password, salt, params)
    }

    /// Stretch `password` with a salt already stored on a wrap or a backup.
    pub(crate) fn from_salt(password: &str, salt: [u8; 16], params: Argon2Params) -> Result<Self> {
        super::progress::set_stage("Deriving key");
        let raw = Zeroizing::new(argon2id(password, &salt, params)?);
        Ok(Self { salt, params, raw })
    }

    /// The salt to store next to anything sealed with [`Self::key`].
    pub(crate) fn salt(&self) -> &[u8; 16] {
        &self.salt
    }

    /// The Argon2id parameters that produced this stretch.
    pub(crate) fn params(&self) -> Argon2Params {
        self.params
    }

    /// A 32-byte key for `info`. Different labels do not run Argon2id again.
    pub(crate) fn key(&self, info: &[u8]) -> Zeroizing<[u8; 32]> {
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(&self.salt), self.raw.as_slice());
        let mut key = Zeroizing::new([0u8; 32]);
        hk.expand(info, key.as_mut())
            .expect("32-byte HKDF output is valid");
        key
    }
}

#[cfg(test)]
thread_local! {
    static ARGON_COUNTING: Cell<bool> = const { Cell::new(false) };
    static ARGON_CALLS: Cell<u64> = const { Cell::new(0) };
}

/// Count [`argon2id`] calls on this thread until the next [`argon_count`] read
/// that a test makes after [`start_argon_count`].
#[cfg(test)]
pub(crate) fn start_argon_count() {
    ARGON_COUNTING.with(|on| on.set(true));
    ARGON_CALLS.with(|n| n.set(0));
}

/// How many times [`argon2id`] has run on this thread since [`start_argon_count`].
#[cfg(test)]
pub(crate) fn argon_count() -> u64 {
    ARGON_CALLS.with(Cell::get)
}
