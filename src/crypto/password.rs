//! The mandatory password: length, a zxcvbn-style strength check, and Argon2id.

use anyhow::{Result, bail};

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

/// Reject a password that is shorter than 12 characters or that zxcvbn scores below 3.
pub fn check_password(password: &str) -> Result<()> {
    if password.chars().count() < 12 {
        bail!("the password must be at least 12 characters, and there is no password reset");
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

/// Argon2id, 32-byte output. `salt` is the full 32-byte value the plan names
/// (`blake3("acp-master" ‖ seed)` for the master secret).
pub fn argon2id(password: &str, salt: &[u8], params: Argon2Params) -> Result<[u8; 32]> {
    use argon2::{Algorithm, Argon2, Params, Version};
    let p = Params::new(params.m_kib, params.t, params.p, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 parameters: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| anyhow::anyhow!("argon2: {e}"))?;
    Ok(out)
}
