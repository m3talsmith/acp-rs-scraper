//! The pinned `acp-id`. Public data only: id, Argon2 parameters, public keys.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

use super::master::AcpId;
use super::password::Argon2Params;
use crate::config::ACP_ID_REL_PATH;

/// What `--identity-init` writes, and what a later wrong-password check reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    /// Pinned account id.
    pub acp_id: AcpId,
    /// Argon2id parameters used when this account was created.
    pub argon2: Argon2Params,
    /// Master ed25519 public key, 32 bytes.
    pub sign_pub: [u8; 32],
    /// Master X25519 public key, 32 bytes. Wraps are sealed to this.
    pub wrap_pub: [u8; 32],
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PinFile {
    acp_id: String,
    argon2_m_kib: u32,
    argon2_t: u32,
    argon2_p: u32,
    sign_pub: String,
    wrap_pub: String,
}

impl Pin {
    /// Path of the pin file under a home directory (`~` is `home`, not `~/.acp`).
    pub fn path(home: &Path) -> PathBuf {
        home.join(ACP_ID_REL_PATH)
    }

    /// Read the pin, if this home has one.
    pub fn load(home: &Path) -> Result<Option<Self>> {
        let path = Self::path(home);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let f: PinFile =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Some(Self {
            acp_id: AcpId::parse(&f.acp_id)?,
            argon2: Argon2Params {
                m_kib: f.argon2_m_kib,
                t: f.argon2_t,
                p: f.argon2_p,
            },
            sign_pub: decode_key(&f.sign_pub)?,
            wrap_pub: decode_key(&f.wrap_pub)?,
        }))
    }

    /// Write the pin (mode `0600`). Refuses to replace an existing account.
    pub fn store(&self, home: &Path) -> Result<()> {
        let path = Self::path(home);
        if path.exists() {
            bail!(
                "an acp-id is already pinned at {}; refusing to overwrite it",
                path.display()
            );
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let f = PinFile {
            acp_id: self.acp_id.to_string(),
            argon2_m_kib: self.argon2.m_kib,
            argon2_t: self.argon2.t,
            argon2_p: self.argon2.p,
            sign_pub: data_encoding::HEXLOWER.encode(&self.sign_pub),
            wrap_pub: data_encoding::HEXLOWER.encode(&self.wrap_pub),
        };
        let body = toml::to_string(&f)?;
        write_private(&path, body.as_bytes())?;
        Ok(())
    }
}

fn decode_key(hex: &str) -> Result<[u8; 32]> {
    let v = data_encoding::HEXLOWER
        .decode(hex.trim().as_bytes())
        .context("public key hex")?;
    let a: [u8; 32] = v
        .as_slice()
        .try_into()
        .context("public key must be 32 bytes")?;
    Ok(a)
}

fn write_private(path: &Path, body: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(body)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, body)?;
    }
    Ok(())
}
