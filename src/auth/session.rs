//! `~/.acp/session.json`: the saved login session, so the next run can skip
//! the one-time code. Written with mode `0600`; cookie values are never logged.
//!
//! reqwest's [`Jar`] cannot be enumerated, but as a [`CookieStore`] it returns
//! the `Cookie` header it would send to a URL. That header (name=value pairs for
//! the site origin) is all a later request needs, so it is what gets saved and
//! later replayed into a fresh jar with [`Jar::add_cookie_str`].

use crate::prelude::*;

/// Contents of the session file.
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct SavedSession {
    /// Site origin the cookies belong to.
    pub base_url: String,
    /// Cookies sent to the origin.
    pub cookies: Vec<SavedCookie>,
}

/// One saved cookie (name and value only; attributes are re-derived on load).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SavedCookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
}

/// `~/.acp/session.json`, under `ACP_HOME` when that is set.
pub fn session_path() -> Result<PathBuf> { Ok(home()?.join(SESSION_REL_PATH)) }

/// Login-window sentence naming the real session file (not a hardcoded `~`).
pub fn session_login_hint() -> String {
    let path = session_path().map(|p| p.display().to_string()).unwrap_or_else(|_| "~/.acp/session.json".into());
    format!("The session is saved in {path} (mode 0600); browsing works offline without it.")
}

fn origin() -> Result<Url> { Ok(Url::parse(base_url())?) }

/// Cookies the jar would send to the site origin.
pub fn jar_cookies(jar: &Jar, url: &Url) -> Vec<SavedCookie> {
    let Some(h) = jar.cookies(url) else { return Vec::new() };
    h.to_str().unwrap_or_default().split(';').filter_map(|kv| {
        let (n, v) = kv.trim().split_once('=')?;
        (!n.is_empty()).then(|| SavedCookie { name: n.into(), value: v.into() })
    }).collect()
}

/// Write the jar's cookies for `url` to `path` (mode `0600`); returns how many were saved.
pub fn save_session_to(path: &Path, jar: &Jar, url: &Url) -> Result<usize> {
    let cookies = jar_cookies(jar, url);
    let s = SavedSession { base_url: url.as_str().trim_end_matches('/').into(), cookies };
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
    let body = serde_json::to_vec_pretty(&s)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(&body)?;
    }
    #[cfg(not(unix))]
    std::fs::write(path, &body)?;
    Ok(s.cookies.len())
}

/// Load saved cookies from `path` into `jar` when they belong to `url`'s origin; returns how many.
pub fn load_session_from(path: &Path, jar: &Jar, url: &Url) -> Result<usize> {
    if !path.exists() { return Ok(0); }
    let s: SavedSession = serde_json::from_str(&std::fs::read_to_string(path)?)
        .with_context(|| format!("reading {}", path.display()))?;
    if s.base_url != url.as_str().trim_end_matches('/') { return Ok(0); }
    for c in &s.cookies {
        jar.add_cookie_str(&format!("{}={}; Path=/", c.name, c.value), url);
    }
    Ok(s.cookies.len())
}

/// Expire every site cookie in `jar` and delete the session file at `path`.
/// Returns whether a file was removed.
pub fn logout_at(path: &Path, jar: &Jar, url: &Url) -> Result<bool> {
    for c in jar_cookies(jar, url) {
        jar.add_cookie_str(&format!("{}=; Path=/; Max-Age=0", c.name), url);
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Save the jar's site cookies to `~/.acp/session.json`.
pub fn save_session(jar: &Jar) -> Result<()> {
    let n = save_session_to(&session_path()?, jar, &origin()?)?;
    verbose!("saved {n} session cookies to {}", session_path()?.display());
    Ok(())
}

/// Load `~/.acp/session.json` into `jar` (no-op when missing).
pub fn load_session(jar: &Jar) -> Result<()> {
    let n = load_session_from(&session_path()?, jar, &origin()?)?;
    if n > 0 { verbose!("loaded {n} session cookies from {}", session_path()?.display()); }
    Ok(())
}

/// Log out locally: clear the site cookies in `jar` and delete `~/.acp/session.json`.
/// The site has no known logout endpoint, so nothing is sent over the network.
pub fn logout(jar: &Jar) -> Result<bool> { logout_at(&session_path()?, jar, &origin()?) }

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("acp-session-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("sub").join("session.json")
    }

    #[test]
    fn save_load_round_trip_with_0600() {
        let url = Url::parse("http://127.0.0.1:4000").unwrap();
        let path = tmp("rt");
        let jar = Jar::default();
        jar.add_cookie_str("_acp_key=abc123; Path=/; HttpOnly", &url);
        jar.add_cookie_str("other=x; Path=/", &url);
        assert_eq!(save_session_to(&path, &jar, &url).unwrap(), 2);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let fresh = Jar::default();
        assert_eq!(load_session_from(&path, &fresh, &url).unwrap(), 2);
        let mut got = jar_cookies(&fresh, &url);
        got.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(got, vec![
            SavedCookie { name: "_acp_key".into(), value: "abc123".into() },
            SavedCookie { name: "other".into(), value: "x".into() },
        ]);
        let elsewhere = Url::parse("https://example.com").unwrap();
        assert_eq!(load_session_from(&path, &Jar::default(), &elsewhere).unwrap(), 0);
    }

    #[test]
    fn logout_clears_jar_and_file() {
        let url = Url::parse("http://127.0.0.1:4000").unwrap();
        let path = tmp("lo");
        let jar = Jar::default();
        jar.add_cookie_str("_acp_key=abc; Path=/", &url);
        save_session_to(&path, &jar, &url).unwrap();
        assert!(logout_at(&path, &jar, &url).unwrap());
        assert!(!path.exists());
        assert!(jar_cookies(&jar, &url).is_empty());
        assert!(!logout_at(&path, &jar, &url).unwrap());
        assert_eq!(load_session_from(&path, &Jar::default(), &url).unwrap(), 0);
    }
}
