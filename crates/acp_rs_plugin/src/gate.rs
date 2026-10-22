//! Unpack a plugin archive and check `package.toml` before anything is loaded.

use data_encoding::HEXLOWER;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Why a package is not loadable. The message never includes file bytes.
#[derive(Debug)]
pub struct GateFailure {
    /// Package name, when the filename or the manifest had one.
    pub name: String,
    /// What failed.
    pub reason: String,
}

impl GateFailure {
    fn new(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self { name: name.into(), reason: reason.into() }
    }
}

/// A package that passed the gate. The library path is inside `root`.
#[derive(Debug)]
pub struct Verified {
    /// `[meta] name`.
    pub name: String,
    /// `[meta] version`.
    pub version: String,
    /// Unpacked tree.
    pub root: PathBuf,
    /// First library, already hashed.
    pub library: PathBuf,
}

/// Host operating system token used in the archive name.
pub fn host_os() -> &'static str {
    match std::env::consts::OS {
        "linux" | "macos" | "windows" => std::env::consts::OS,
        other => other,
    }
}

/// Host architecture token used in the archive name.
pub fn host_arch() -> &'static str {
    std::env::consts::ARCH
}

/// Unpack `archive` into `dest` and check the manifest, paths, and sha256 fields.
pub fn verify_archive(archive: &Path, dest: &Path) -> Result<Verified, GateFailure> {
    let filename = archive
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("plugin")
        .to_string();
    let named = split_filename(&filename).map_err(|r| GateFailure::new(filename.clone(), r))?;
    if named.os != host_os() || named.arch != host_arch() {
        return Err(GateFailure::new(
            named.name,
            format!("package is {}-{}, this host is {}-{}", named.os, named.arch, host_os(), host_arch()),
        ));
    }
    fs::create_dir_all(dest).map_err(|e| GateFailure::new(named.name.clone(), format!("unpack directory: {e}")))?;
    unpack(archive, dest).map_err(|r| GateFailure::new(named.name.clone(), r))?;
    check_tree(dest).map_err(|r| GateFailure::new(named.name.clone(), r))?;
    let manifest = fs::read_to_string(dest.join("package.toml"))
        .map_err(|_| GateFailure::new(named.name.clone(), "package.toml is missing"))?;
    let meta = parse_manifest(&manifest).map_err(|r| GateFailure::new(named.name.clone(), r))?;
    if meta.name != named.name || meta.version != named.version {
        return Err(GateFailure::new(
            meta.name.clone(),
            format!("filename is {}-{}, package.toml says {}-{}", named.name, named.version, meta.name, meta.version),
        ));
    }
    if let Some(license) = &meta.license {
        let text = fs::read_to_string(dest.join("LICENSE"))
            .map_err(|_| GateFailure::new(meta.name.clone(), "license is set but LICENSE is missing"))?;
        if !text.contains(license) {
            return Err(GateFailure::new(meta.name, "LICENSE does not match the license field"));
        }
    }
    let mut library = None;
    for entry in meta.libraries.iter().chain(meta.binaries.iter()) {
        let path = resolve_inside(dest, &entry.path).map_err(|r| GateFailure::new(meta.name.clone(), r))?;
        let got = sha256_file(&path).map_err(|r| GateFailure::new(meta.name.clone(), r))?;
        if got != entry.sha256 {
            return Err(GateFailure::new(
                meta.name,
                format!("sha256 mismatch for {}", entry.path),
            ));
        }
        if entry.library && library.is_none() {
            library = Some(path);
        }
    }
    let Some(library) = library else {
        return Err(GateFailure::new(meta.name, "package has no library"));
    };
    Ok(Verified { name: meta.name, version: meta.version, root: dest.to_path_buf(), library })
}

struct Named {
    name: String,
    os: String,
    arch: String,
    version: String,
}

fn split_filename(filename: &str) -> Result<Named, String> {
    let stem = filename.strip_suffix(".tar.gz").ok_or_else(|| "archive name must end in .tar.gz".to_string())?;
    let parts: Vec<&str> = stem.split('-').collect();
    for i in 0..parts.len() {
        if i + 2 >= parts.len() { break; }
        if !is_os(parts[i]) || !is_arch(parts[i + 1]) { continue; }
        let name = parts[..i].join("-");
        if name.is_empty() { continue; }
        let version = parts[i + 2..].join("-");
        check_semver(&version)?;
        return Ok(Named { name, os: parts[i].into(), arch: parts[i + 1].into(), version });
    }
    Err("archive name must be <name>-<os>-<arch>-<version>.tar.gz".into())
}

fn is_os(s: &str) -> bool { matches!(s, "linux" | "macos" | "windows") }
fn is_arch(s: &str) -> bool { matches!(s, "x86_64" | "aarch64" | "arm" | "i686") }

fn check_semver(v: &str) -> Result<(), String> {
    let core = v.split_once('-').map(|(c, _)| c).unwrap_or(v);
    let core = core.split_once('+').map(|(c, _)| c).unwrap_or(core);
    let nums: Vec<&str> = core.split('.').collect();
    if nums.len() != 3 || nums.iter().any(|n| n.is_empty() || !n.chars().all(|c| c.is_ascii_digit())) {
        return Err(format!("version {v} is not a quoted semantic version"));
    }
    Ok(())
}

fn unpack(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|e| format!("reading archive: {e}"))?;
    let dec = GzDecoder::new(file);
    let mut tar = tar::Archive::new(dec);
    let entries = tar.entries().map_err(|e| format!("not a gzipped tar: {e}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("not a gzipped tar: {e}"))?;
        let rel = entry.path().map_err(|e| format!("bad path: {e}"))?.into_owned();
        reject_path(&rel)?;
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            let target = entry.link_name().map_err(|e| format!("symlink: {e}"))?.unwrap_or_default();
            reject_path(&target)?;
            let joined = rel.parent().unwrap_or(Path::new("")).join(&target);
            reject_path(&joined)?;
        }
        entry.unpack_in(dest).map_err(|e| format!("unpack: {e}"))?;
    }
    Ok(())
}

fn reject_path(path: &Path) -> Result<(), String> {
    if path.is_absolute() {
        return Err(format!("absolute path {}", path.display()));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!("path escapes the archive: {}", path.display()));
    }
    Ok(())
}

fn check_tree(root: &Path) -> Result<(), String> {
    let canon = root.canonicalize().map_err(|e| format!("unpack tree: {e}"))?;
    walk(&canon, &canon)
}

fn walk(root: &Path, dir: &Path) -> Result<(), String> {
    for ent in fs::read_dir(dir).map_err(|e| format!("reading {}: {e}", dir.display()))? {
        let ent = ent.map_err(|e| e.to_string())?;
        let path = ent.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(|e| e.to_string())?;
            let resolved = if target.is_absolute() {
                return Err(format!("symlink leaves the archive: {}", path.display()));
            } else {
                path.parent().unwrap_or(root).join(target)
            };
            if resolved.components().any(|c| matches!(c, Component::ParentDir)) {
                return Err(format!("symlink leaves the archive: {}", path.display()));
            }
        }
        if meta.is_dir() && !meta.file_type().is_symlink() {
            walk(root, &path)?;
        }
    }
    Ok(())
}

fn resolve_inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = Path::new(rel);
    reject_path(rel)?;
    let path = root.join(rel);
    if !path.is_file() {
        return Err(format!("missing {}", rel.display()));
    }
    let canon_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canon = path.canonicalize().map_err(|e| format!("{}: {e}", rel.display()))?;
    if !canon.starts_with(&canon_root) {
        return Err(format!("path escapes the archive: {}", rel.display()));
    }
    Ok(path)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(HEXLOWER.encode(&hasher.finalize()))
}

struct Entry {
    path: String,
    sha256: String,
    library: bool,
}

struct Meta {
    name: String,
    version: String,
    license: Option<String>,
    binaries: Vec<Entry>,
    libraries: Vec<Entry>,
}

fn parse_manifest(text: &str) -> Result<Meta, String> {
    let value: toml::Value = toml::from_str(text).map_err(|e| format!("package.toml: {e}"))?;
    let table = value.as_table().ok_or("package.toml is not a table")?;
    let meta = table.get("meta").and_then(|v| v.as_table()).ok_or("package.toml needs [meta]")?;
    let name = req_str(meta, "name")?;
    let version = match meta.get("version") {
        Some(toml::Value::String(s)) => s.clone(),
        Some(_) => return Err("version must be a quoted string".into()),
        None => return Err("version is missing".into()),
    };
    check_semver(&version)?;
    if let Some(email) = optional_author_email(meta)? {
        if !email_ok(&email) {
            return Err("author email is not an email address".into());
        }
    }
    if let Some(doc) = optional_str(meta, "documentation")? {
        check_uri_or_node(&doc)?;
    }
    if let Some(src) = optional_str(meta, "source")? {
        check_uri_or_node(&src)?;
    }
    let license = optional_str(meta, "license")?;
    let install = table.get("install").and_then(|v| v.as_table()).ok_or("package.toml needs [install]")?;
    let binaries = entries(install, "binaries", false)?;
    let libraries = entries(install, "libraries", true)?;
    Ok(Meta { name, version, license, binaries, libraries })
}

fn req_str(t: &toml::Table, key: &str) -> Result<String, String> {
    match t.get(key) {
        Some(toml::Value::String(s)) if !s.is_empty() => Ok(s.clone()),
        Some(toml::Value::String(_)) => Err(format!("{key} is empty")),
        Some(_) => Err(format!("{key} must be a string")),
        None => Err(format!("{key} is missing")),
    }
}

fn optional_str(t: &toml::Table, key: &str) -> Result<Option<String>, String> {
    match t.get(key) {
        None => Ok(None),
        Some(toml::Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

fn optional_author_email(meta: &toml::Table) -> Result<Option<String>, String> {
    let Some(authors) = meta.get("authors") else { return Ok(None) };
    let list = authors.as_array().ok_or("authors must be a list")?;
    for author in list {
        let Some(table) = author.as_table() else {
            return Err("an author entry must be a table".into());
        };
        if let Some(email) = table.get("email") {
            let toml::Value::String(email) = email else {
                return Err("author email must be a string".into());
            };
            return Ok(Some(email.clone()));
        }
    }
    Ok(None)
}

fn email_ok(email: &str) -> bool {
    let Some((local, host)) = email.split_once('@') else { return false };
    !local.is_empty() && host.contains('.') && !email.contains(' ')
}

fn check_uri_or_node(value: &str) -> Result<(), String> {
    if value.starts_with("https://") || value.starts_with("http://") || value.starts_with("acp:") {
        if value.contains(' ') { return Err(format!("{value} is not a URI or a node address")); }
        return Ok(());
    }
    Err(format!("{value} is not a URI or a node address"))
}

fn entries(install: &toml::Table, key: &str, library: bool) -> Result<Vec<Entry>, String> {
    let Some(value) = install.get(key) else { return Ok(Vec::new()) };
    let list = value.as_array().ok_or_else(|| format!("{key} must be a list"))?;
    let mut out = Vec::new();
    for item in list {
        let table = item.as_table().ok_or_else(|| format!("a {key} entry must be a table"))?;
        let path = req_str(table, "path")?;
        let sha256 = match table.get("sha256") {
            Some(toml::Value::String(s)) => s.clone(),
            Some(_) => return Err(format!("sha256 for {path} must be a string")),
            None => return Err(format!("sha256 is missing for {path}")),
        };
        if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
            return Err(format!("sha256 for {path} must be 64 lowercase hex digits"));
        }
        out.push(Entry { path, sha256, library });
    }
    Ok(out)
}

/// sha256 of `bytes`, lowercase hex.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    HEXLOWER.encode(&Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    fn tar_gz(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        {
            let enc = GzEncoder::new(&mut body, Compression::default());
            let mut builder = tar::Builder::new(enc);
            for (path, data) in files {
                let mut header = tar::Header::new_gnu();
                header.set_path(path).unwrap();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append(&header, *data).unwrap();
            }
            builder.finish().unwrap();
        }
        body
    }

    fn manifest(sha: &str) -> String {
        format!(
            r#"[meta]
name = "example"
version = "1.2.3"
license = "MIT"
authors = [{{ name = "Ada", email = "ada@example.com" }}]

[install]
libraries = [
  {{ name = "example", path = "lib/libexample.so", sha256 = "{sha}" }}
]
"#
        )
    }

    fn place(dir: &Path, bytes: &[u8]) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let os = host_os();
        let arch = host_arch();
        let path = dir.join(format!("example-{os}-{arch}-1.2.3.tar.gz"));
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_matching_package_passes_and_a_bad_hash_does_not() {
        let lib = b"not a real library";
        let sha = sha256_bytes(lib);
        let bytes = tar_gz(&[
            ("package.toml", manifest(&sha).as_bytes()),
            ("LICENSE", b"MIT"),
            ("lib/libexample.so", lib),
        ]);
        let tmp = std::env::temp_dir().join(format!("acp-gate-ok-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let archive = place(&tmp, &bytes);
        let dest = tmp.join("out");
        let got = verify_archive(&archive, &dest).unwrap();
        assert_eq!(got.name, "example");
        assert_eq!(got.version, "1.2.3");
        assert!(got.library.ends_with("libexample.so"));

        let bad = tar_gz(&[
            ("package.toml", manifest(&"ab".repeat(32)).as_bytes()),
            ("LICENSE", b"MIT"),
            ("lib/libexample.so", lib),
        ]);
        let archive = place(&tmp, &bad);
        let err = verify_archive(&archive, &tmp.join("bad")).unwrap_err();
        assert!(err.reason.contains("sha256 mismatch"), "{}", err.reason);
        assert!(!err.reason.contains("not a real library"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn a_parent_path_and_a_bare_version_and_a_bad_email_fail() {
        let err = reject_path(Path::new("../outside")).unwrap_err();
        assert!(err.contains("escapes"), "{err}");
        assert!(reject_path(Path::new("/etc/passwd")).unwrap_err().contains("absolute"));
        let bare = "[meta]\nname = \"example\"\nversion = 1.2.3\n[install]\nlibraries = []\n";
        assert!(parse_manifest(bare).is_err());
        let email = manifest(&"ab".repeat(32)).replace("ada@example.com", "not-an-email");
        let err = parse_manifest(&email).err().unwrap();
        assert!(err.contains("email"), "{err}");
        assert!(!err.contains("not-an-email"), "{err}");
    }

    #[test]
    fn the_filename_must_match_the_host_and_the_manifest() {
        let err = split_filename("example-linux-x86_64-1.2.3.tar.gz");
        assert!(err.is_ok());
        assert!(split_filename("example-1.2.3.tar.gz").is_err());
        assert!(check_semver("1.2").is_err());
        assert!(check_semver("1.2.3-alpha").is_ok());
    }
}
