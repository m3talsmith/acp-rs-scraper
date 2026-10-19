//! Password-wrapped copy of the 24-word phrase entropy.
//!
//! `~/.acp/recovery.seal` holds the 256-bit BIP-39 entropy, sealed with
//! XChaCha20-Poly1305 under HKDF(Argon2id(password)). The words themselves are
//! not stored and are not logged. The password alone cannot invent them: without
//! this file, the paper copy, or three SLIP-39 shares, there is nothing to show.

use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use ciborium::Value;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use super::password::{Argon2Params, Stretch};
use crate::config::{RECOVERY_NEXT_REL_PATH, RECOVERY_SEAL_REL_PATH};

/// Plain-language steps for the recovery phrase. Shown after encryption, in the
/// Help screen, and in `--help`.
pub const GUIDE: &str = include_str!("recovery_guide.txt");

const VERSION: u64 = 1;
const INFO: &[u8] = b"acp-recovery-phrase-v1";

/// Whether `~/.acp/recovery-next` is present. The file is a reminder, not a secret.
pub(crate) fn next_pending(home: &Path) -> bool {
    home.join(RECOVERY_NEXT_REL_PATH).is_file()
}

/// Remember that encryption just finished and the next steps have not been shown.
pub(crate) fn mark_next(home: &Path) -> Result<()> {
    write_file(
        home,
        RECOVERY_NEXT_REL_PATH,
        b"recovery steps not shown yet\n",
    )
}

/// The next-step reminder has been shown, or the phrase has been created.
pub(crate) fn clear_next(home: &Path) -> Result<()> {
    let path = home.join(RECOVERY_NEXT_REL_PATH);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Whether an encrypted copy of the phrase entropy is on disk.
pub(crate) fn seal_exists(home: &Path) -> bool {
    seal_path(home).is_file()
}

/// Path of the encrypted phrase (`~/.acp/recovery.seal`).
pub(crate) fn seal_path(home: &Path) -> PathBuf {
    home.join(RECOVERY_SEAL_REL_PATH)
}

/// Seal `entropy` under a new salt for `password`.
pub(crate) fn seal(
    home: &Path,
    password: &str,
    params: Argon2Params,
    entropy: &[u8; 32],
) -> Result<()> {
    let stretch = Stretch::fresh(password, params)?;
    write_seal(home, &stretch, entropy)
}

/// Open the seal. A wrong password fails the authenticator and writes nothing.
pub(crate) fn open(home: &Path, password: &str) -> Result<Zeroizing<[u8; 32]>> {
    let path = seal_path(home);
    if !path.is_file() {
        bail!(
            "the recovery phrase was not saved on this computer, and the password cannot rebuild it. \
             Use the paper copy, or run --shares-print with at least 3 shares and the password. \
             A phrase created before this file existed was shown once and not stored."
        );
    }
    let file = read_seal(&path)?;
    let stretch = Stretch::from_salt(password, file.salt, file.params)?;
    let plain = decrypt(&stretch.key(INFO), &file.nonce, &file.wrapped)?;
    if plain.len() != 32 {
        bail!("the recovery phrase has the wrong length");
    }
    let mut entropy = Zeroizing::new([0u8; 32]);
    entropy.copy_from_slice(&plain);
    Ok(entropy)
}

/// What [`reseal`] did with the on-disk phrase.
pub(crate) enum Reseal {
    /// There is no seal to move.
    Absent,
    /// The seal was rewritten under `fresh`. `previous` is the old file.
    Moved { previous: Vec<u8> },
    /// The seal did not open with this password, so it was left as it was.
    Kept,
}

/// Rewrite the seal under `fresh` when it opens with `old_password`.
/// The new password is not stretched again: `fresh` is the stretch already
/// computed for the database password change.
pub(crate) fn reseal(home: &Path, old_password: &str, fresh: &Stretch) -> Result<Reseal> {
    let path = seal_path(home);
    if !path.is_file() {
        return Ok(Reseal::Absent);
    }
    let previous = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let entropy = match open(home, old_password) {
        Ok(entropy) => entropy,
        Err(err) if password_rejected(&err) => return Ok(Reseal::Kept),
        Err(err) => return Err(err),
    };
    super::progress::set_stage("Saving the recovery phrase");
    write_seal(home, fresh, &entropy)?;
    Ok(Reseal::Moved { previous })
}

/// Put a previous seal back. Used when a later step of a password change fails.
pub(crate) fn restore_bytes(home: &Path, bytes: &[u8]) -> Result<()> {
    write_file(home, RECOVERY_SEAL_REL_PATH, bytes)
}

/// Clear the screen and, where the terminal allows it, the scrollback.
pub(crate) fn clear_terminal() -> Result<()> {
    use std::io::Write;
    let mut out = std::io::stdout();
    write!(out, "\x1b[2J\x1b[3J\x1b[H")?;
    out.flush()?;
    Ok(())
}

fn password_rejected(err: &anyhow::Error) -> bool {
    err.to_string()
        .contains("does not open the recovery phrase")
}

fn write_seal(home: &Path, stretch: &Stretch, entropy: &[u8; 32]) -> Result<()> {
    let mut nonce = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let wrapped = encrypt(&stretch.key(INFO), &nonce, entropy)?;
    let params = stretch.params();
    let value = Value::Array(vec![
        Value::Integer(VERSION.into()),
        Value::Integer(u64::from(params.m_kib).into()),
        Value::Integer(u64::from(params.t).into()),
        Value::Integer(u64::from(params.p).into()),
        Value::Bytes(stretch.salt().to_vec()),
        Value::Bytes(nonce.to_vec()),
        Value::Bytes(wrapped),
    ]);
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&value, &mut buf)?;
    write_file(home, RECOVERY_SEAL_REL_PATH, &buf)
}

struct SealFile {
    params: Argon2Params,
    salt: [u8; 16],
    nonce: [u8; 24],
    wrapped: Vec<u8>,
}

fn read_seal(path: &Path) -> Result<SealFile> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let value: Value = ciborium::de::from_reader(bytes.as_slice()).context("recovery phrase")?;
    let Value::Array(a) = value else {
        bail!("the recovery phrase file is not an array");
    };
    if a.len() != 7 {
        bail!("the recovery phrase file has {} fields", a.len());
    }
    let version = int_field(&a[0])?;
    if version > VERSION {
        bail!("this recovery phrase uses version {version}, and this build reads up to {VERSION}");
    }
    let salt = bytes_field(&a[4], 16)?;
    let nonce = bytes_field(&a[5], 24)?;
    let wrapped = match &a[6] {
        Value::Bytes(b) => b.clone(),
        _ => bail!("the recovery phrase wrap is not bytes"),
    };
    let mut salt_a = [0u8; 16];
    let mut nonce_a = [0u8; 24];
    salt_a.copy_from_slice(&salt);
    nonce_a.copy_from_slice(&nonce);
    Ok(SealFile {
        params: Argon2Params {
            m_kib: int_field(&a[1])? as u32,
            t: int_field(&a[2])? as u32,
            p: int_field(&a[3])? as u32,
        },
        salt: salt_a,
        nonce: nonce_a,
        wrapped,
    })
}

fn encrypt(key: &Zeroizing<[u8; 32]>, nonce: &[u8; 24], plain: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new((&**key).into());
    cipher
        .encrypt(
            nonce.into(),
            Payload {
                msg: plain,
                aad: INFO,
            },
        )
        .map_err(|_| anyhow::anyhow!("sealing the recovery phrase failed"))
}

fn decrypt(key: &Zeroizing<[u8; 32]>, nonce: &[u8; 24], body: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new((&**key).into());
    cipher
        .decrypt(
            nonce.into(),
            Payload {
                msg: body,
                aad: INFO,
            },
        )
        .map_err(|_| {
            anyhow::anyhow!("the password does not open the recovery phrase. Nothing was written.")
        })
}

fn write_file(home: &Path, rel: &str, body: &[u8]) -> Result<()> {
    let path = home.join(rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;
    use std::io::Write;
    file.write_all(body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn int_field(v: &Value) -> Result<u64> {
    match v {
        Value::Integer(i) => i64::try_from(*i)
            .ok()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| anyhow::anyhow!("integer is out of range")),
        _ => bail!("expected an integer"),
    }
}

fn bytes_field(v: &Value, len: usize) -> Result<Vec<u8>> {
    match v {
        Value::Bytes(b) if b.len() == len => Ok(b.clone()),
        _ => bail!("expected {len} bytes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "correct horse battery staple";

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "acp-seal-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_guide_names_the_commands_and_the_share_threshold() {
        assert!(GUIDE.contains("not the master key"));
        assert!(GUIDE.contains("3 of 5") || GUIDE.contains("Any 3 of those 5"));
        assert!(GUIDE.contains("--show-phrase"));
        assert!(GUIDE.contains("--identity-init"));
        assert!(GUIDE.contains("--shares-print"));
        assert!(GUIDE.contains("--shares-verify"));
        assert!(GUIDE.contains("--restore"));
        assert!(GUIDE.contains(".acpbackup"));
        assert!(GUIDE.contains("Argon2id"));
        assert!(GUIDE.contains("scrollback"));
    }

    #[test]
    fn a_seal_round_trips_and_a_wrong_password_is_refused() {
        let home = scratch("round");
        let entropy = [0x11u8; 32];
        seal(&home, PASSWORD, Argon2Params::FAST, &entropy).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(seal_path(&home))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        let opened = open(&home, PASSWORD).unwrap();
        assert_eq!(opened.as_slice(), &entropy);
        let phrase = super::super::master::phrase_from_entropy(&opened).unwrap();
        assert_eq!(phrase.split_whitespace().count(), 24);
        let err = open(&home, "correct horse battery staple!").unwrap_err();
        assert!(password_rejected(&err), "{err}");
        assert!(!err.to_string().contains("correct horse battery staple!"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn reseal_moves_the_phrase_onto_the_new_stretch() {
        let home = scratch("reseal");
        let entropy = [0x22u8; 32];
        seal(&home, PASSWORD, Argon2Params::FAST, &entropy).unwrap();
        let next = "correct horse battery staple and more";
        let fresh = Stretch::fresh(next, Argon2Params::FAST).unwrap();
        let moved = reseal(&home, PASSWORD, &fresh).unwrap();
        assert!(matches!(moved, Reseal::Moved { .. }));
        assert_eq!(open(&home, next).unwrap().as_slice(), &entropy);
        assert!(open(&home, PASSWORD).is_err());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_seal_under_a_different_password_is_left_in_place() {
        let home = scratch("kept");
        let entropy = [0x33u8; 32];
        let other = "a different passphrase for the phrase";
        seal(&home, other, Argon2Params::FAST, &entropy).unwrap();
        let before = std::fs::read(seal_path(&home)).unwrap();
        let fresh = Stretch::fresh(PASSWORD, Argon2Params::FAST).unwrap();
        let kept = reseal(&home, PASSWORD, &fresh).unwrap();
        assert!(matches!(kept, Reseal::Kept));
        assert_eq!(std::fs::read(seal_path(&home)).unwrap(), before);
        assert_eq!(open(&home, other).unwrap().as_slice(), &entropy);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn restoring_the_previous_bytes_undoes_a_reseal() {
        let home = scratch("undo");
        let entropy = [0x44u8; 32];
        seal(&home, PASSWORD, Argon2Params::FAST, &entropy).unwrap();
        let previous = std::fs::read(seal_path(&home)).unwrap();
        let fresh =
            Stretch::fresh("correct horse battery staple and more", Argon2Params::FAST).unwrap();
        assert!(matches!(
            reseal(&home, PASSWORD, &fresh).unwrap(),
            Reseal::Moved { .. }
        ));
        restore_bytes(&home, &previous).unwrap();
        assert_eq!(open(&home, PASSWORD).unwrap().as_slice(), &entropy);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn the_next_step_marker_can_be_set_and_cleared() {
        let home = scratch("mark");
        assert!(!next_pending(&home));
        mark_next(&home).unwrap();
        assert!(next_pending(&home));
        clear_next(&home).unwrap();
        assert!(!next_pending(&home));
        clear_next(&home).unwrap();
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn opening_without_a_seal_names_the_paper_copy() {
        let home = scratch("none");
        let err = open(&home, PASSWORD).unwrap_err().to_string();
        assert!(err.contains("not saved"), "{err}");
        assert!(err.contains("--shares-print"), "{err}");
        let _ = std::fs::remove_dir_all(&home);
    }
}
