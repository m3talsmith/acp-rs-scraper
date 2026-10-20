//! Downloading theme files from the repo's `themes/` folder at this version's tag
//! ([`THEME_DOWNLOAD_BASE`], or [`THEME_URL_ENV`] in tests). The first run with an empty
//! `~/.acp/themes` fetches `default.toml` in the background; `--theme-init` and
//! *Theme: init* fetch on request. Each download is capped at [`THEME_MAX_BYTES`] and
//! [`THEME_DOWNLOAD_TIMEOUT`], must parse as a theme, sends no cookies, and is written
//! atomically (mode 0600) without replacing an existing file.

use super::*;
use ratatui::style::Color;

/// What a download did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fetched {
    /// The file was written.
    Written(PathBuf),
    /// A file of that name was already there; it was kept and nothing was replaced.
    Kept(PathBuf),
}

impl Fetched {
    /// The theme file.
    pub fn path(&self) -> &Path { match self { Fetched::Written(p) | Fetched::Kept(p) => p } }
}

/// Whether `ACP_THEME_DOWNLOAD` turns the automatic download off.
pub fn env_off(v: Option<&str>) -> bool {
    matches!(v.map(|v| v.trim().to_ascii_lowercase()).as_deref(), Some("off" | "0" | "false" | "no"))
}

/// Whether the first run may download the default theme: not `ACP_THEME_DOWNLOAD=off`
/// and not `[ui] theme_download = false`.
pub fn auto_enabled(env: Option<&str>, prefs: &Prefs) -> bool {
    !env_off(env) && prefs.ui.as_ref().and_then(|u| u.theme_download) != Some(false)
}

/// [`auto_enabled`] for this process.
pub fn auto_enabled_now(prefs: &Prefs) -> bool { auto_enabled(std::env::var(THEME_DOWNLOAD_ENV).ok().as_deref(), prefs) }

/// Fetch `url` with the program's HTTP client and an empty cookie jar (no cookies are
/// sent), refusing bodies over [`THEME_MAX_BYTES`] and giving up after `timeout`.
pub async fn fetch(url: &str, timeout: std::time::Duration) -> Result<Vec<u8>> {
    let http = crate::http::client(Arc::new(Jar::default()))?;
    let go = async {
        let mut r = http.get(url).header(reqwest::header::ACCEPT, "text/plain, application/toml, */*").send().await
            .with_context(|| format!("fetching {url}"))?;
        if !r.status().is_success() { bail!("{url} answered {}", r.status()); }
        let too_big = || anyhow!("{url} is over {} KiB; theme files are at most that", THEME_MAX_BYTES / 1024);
        if r.content_length().is_some_and(|n| n > THEME_MAX_BYTES) { return Err(too_big()); }
        let mut buf = Vec::new();
        while let Some(c) = r.chunk().await.with_context(|| format!("reading {url}"))? {
            buf.extend_from_slice(&c);
            if buf.len() as u64 > THEME_MAX_BYTES { return Err(too_big()); }
        }
        Ok(buf)
    };
    match tokio::time::timeout(timeout, go).await {
        Ok(r) => r,
        Err(_) => bail!("no answer from {url} within {} s (timed out)", timeout.as_secs_f64()),
    }
}

/// Check that `bytes` is a usable theme and write it as `dir/<name>.toml` (atomic,
/// mode 0600), keeping a file that is already there.
pub fn store(dir: &Path, name: &str, bytes: &[u8]) -> Result<Fetched> {
    let path = theme_path(dir, name).map_err(|e| anyhow!(e))?;
    if bytes.len() as u64 > THEME_MAX_BYTES { bail!("the downloaded {name}.toml is over {} KiB", THEME_MAX_BYTES / 1024); }
    let src = std::str::from_utf8(bytes).map_err(|_| anyhow!("the downloaded {name}.toml is not UTF-8 text"))?;
    parse(src, name).map_err(|e| anyhow!("the downloaded {name}.toml is not a valid theme: {e}"))?;
    if atomic_write_new(&path, bytes).with_context(|| format!("writing {}", path.display()))? { Ok(Fetched::Written(path)) } else { Ok(Fetched::Kept(path)) }
}

/// Download theme `name` from `url` into `dir`. An existing file is kept without
/// asking the network.
pub async fn download_from(dir: &Path, name: &str, url: &str, timeout: std::time::Duration) -> Result<Fetched> {
    let path = theme_path(dir, name).map_err(|e| anyhow!(e))?;
    if path.exists() { return Ok(Fetched::Kept(path)); }
    let bytes = fetch(url, timeout).await?;
    store(dir, name, &bytes)
}

/// Download theme `name` from [`theme_download_url`] into `dir`.
pub async fn download(dir: &Path, name: &str) -> Result<Fetched> {
    download_from(dir, name, &theme_download_url(name), THEME_DOWNLOAD_TIMEOUT).await
}

/// What the user sees when theme `name` could not be downloaded, and how to fix it.
pub fn failure_notice(dir: &Path, name: &str, err: &str) -> String {
    format!("could not download the {name} theme: {err}. The terminal UI uses your terminal's own colours until a theme \
        is there. To fix it, check the connection and run `acp-red-star --theme-init` (or Theme: init in the command \
        palette), save {} as {}, or write a minimal starter theme instead with `acp-red-star --theme-starter NAME` \
        (Theme: starter in the palette, or Create starter theme in the F5 list).", theme_download_url(name), dir.join(format!("{name}.toml")).display())
}

/// What the user sees when the first run did not download the default because it is turned off.
pub fn off_notice(dir: &Path) -> String {
    format!("no theme files in {} and the automatic theme download is off ({THEME_DOWNLOAD_ENV}=off or [ui] theme_download = false); \
        using the terminal's own colours. Run `acp-red-star --theme-init` to download the default theme, or create one with Shift-F5.",
        dir.display())
}

/// What the user sees after a theme was downloaded.
pub fn done_notice(name: &str, f: &Fetched) -> String {
    match f {
        Fetched::Written(p) => format!("downloaded the {name} theme into {}", p.display()),
        Fetched::Kept(p) => format!("{} already exists; it was kept (nothing downloaded)", p.display()),
    }
}

/// The minimal starter theme: every palette colour set to a plain terminal colour
/// (the 16 ANSI colours and `reset`), no element overrides, one variant (light uses
/// it too). Generated here, not a copy of any shipped theme; edit it with Shift-F5.
pub fn starter(name: &str) -> ThemeFile {
    let mut f = ThemeFile::empty(name);
    f.description = "Starter theme: plain terminal colours (edit it with Shift-F5)".into();
    for (k, v) in [("accent", Color::Red), ("muted", Color::DarkGray), ("border", Color::DarkGray), ("on_accent", Color::White),
        ("fg", Color::Reset), ("bg", Color::Reset)] {
        f.dark.set(k, ColorSpec::Value(v));
    }
    f
}

/// Write [`starter`] as `dir/<name>.toml` (atomic, mode 0600), keeping a file that is
/// already there. Never touches the network.
pub fn write_starter(dir: &Path, name: &str) -> Result<Fetched> {
    let path = theme_path(dir, name).map_err(|e| anyhow!(e))?;
    let text = to_toml(&starter(name));
    if atomic_write_new(&path, text.as_bytes()).with_context(|| format!("writing {}", path.display()))? { Ok(Fetched::Written(path)) } else { Ok(Fetched::Kept(path)) }
}

/// A free name for a starter theme: `base`, else `base-2`, `base-3`…
pub fn free_name(dir: &Path, base: &str) -> String {
    (1..1000).map(|i| if i == 1 { base.to_string() } else { format!("{base}-{i}") })
        .find(|n| !dir.join(format!("{n}.toml")).exists()).unwrap_or_else(|| base.to_string())
}

/// What the user sees after a starter theme was written (or kept).
pub fn starter_notice(name: &str, f: &Fetched) -> String {
    match f {
        Fetched::Written(p) => format!("wrote the starter theme {name} to {}; pick it with F5 or edit it with Shift-F5", p.display()),
        Fetched::Kept(p) => format!("{} already exists; it was kept (no starter written)", p.display()),
    }
}
