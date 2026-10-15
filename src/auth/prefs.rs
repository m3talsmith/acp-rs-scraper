//! `~/.acp/preferences.toml`: the remembered login email.

use crate::prelude::*;

#[derive(Serialize, Deserialize, Default)]
/// Contents of `~/.acp/preferences.toml`.
pub struct Prefs {
    /// Login email.
    pub email: Option<String>,
}

/// Read `email` from `~/.acp/preferences.toml`, prompting for it and creating the file when missing.
pub fn load_email() -> Result<String> {
    let path = home()?.join(PREFS_REL_PATH);
    if path.exists() {
        let p: Prefs = toml::from_str(&std::fs::read_to_string(&path)?)?;
        if let Some(e) = p.email.filter(|e| !e.is_empty()) {
            return Ok(e);
        }
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    let email = prompt("Email: ")?;
    std::fs::write(&path, toml::to_string(&Prefs { email: Some(email.clone()) })?)?;
    Ok(email)
}
