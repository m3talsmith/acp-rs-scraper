//! Host side of the package gate.
//!
//! A failed archive moves to the quarantine directory. A rustc or
//! `acp_rs_plugin` mismatch is refused by [`crate::service::admit`] and is
//! not moved. A package that passes stays where it was and stays off until
//! the user enables it. This module does not run on startup.

use std::fs;
use std::path::{Path, PathBuf};

use acp_rs_plugin::{verify_archive, Verified};

/// Check `archive`. On failure, move it into `quarantine_dir` and write the reason beside it.
pub fn gate_or_quarantine(archive: &Path, quarantine_dir: &Path) -> Result<Verified, String> {
    let unpack = unpack_dir(archive);
    let _ = fs::remove_dir_all(&unpack);
    match verify_archive(archive, &unpack) {
        Ok(verified) => Ok(verified),
        Err(err) => {
            fs::create_dir_all(quarantine_dir).map_err(|e| e.to_string())?;
            let name = archive
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("plugin.tar.gz");
            let dest = quarantine_dir.join(name);
            fs::rename(archive, &dest).map_err(|e| e.to_string())?;
            fs::write(quarantine_dir.join(format!("{name}.reason")), &err.reason)
                .map_err(|e| e.to_string())?;
            let _ = fs::remove_dir_all(&unpack);
            Err(err.reason)
        }
    }
}

fn unpack_dir(archive: &Path) -> PathBuf {
    let mut path = archive.to_path_buf();
    path.set_extension("unpack");
    path
}

/// What one scan of a plugins directory did.
///
/// A passing archive is not enabled. This scan does not `dlopen`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReloadReport {
    /// Filenames that passed the gate and stayed in place.
    pub passed: Vec<String>,
    /// Filenames moved into the quarantine directory.
    pub quarantined: Vec<String>,
}

/// Scan `plugins_dir` and run the package gate. This is what `plugins.reload` calls.
///
/// A missing directory is an empty report. A failed archive is logged as a
/// potential security threat and moved into `quarantine_dir`. A rustc or
/// `acp_rs_plugin` mismatch is not this function: that refusal is
/// [`crate::service::admit`], and it does not move the archive. This scan
/// does not run on startup, and it is not bound to a key.
pub fn reload(plugins_dir: &Path, quarantine_dir: &Path) -> ReloadReport {
    let mut report = ReloadReport::default();
    let entries = match fs::read_dir(plugins_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return report,
        Err(err) => {
            crate::logging::log_error(format_args!(
                "potential security threat: reading {}: {err}",
                plugins_dir.display()
            ));
            return report;
        }
    };
    let mut archives: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".tar.gz"))
        })
        .collect();
    archives.sort();
    for archive in archives {
        let name = archive
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin.tar.gz")
            .to_string();
        match gate_or_quarantine(&archive, quarantine_dir) {
            Ok(_) => report.passed.push(name),
            Err(reason) => {
                crate::logging::log_error(format_args!(
                    "potential security threat: plugin {name}: {reason}"
                ));
                report.quarantined.push(name);
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_rs_plugin::{host_arch, host_os, sha256_bytes};
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

    #[test]
    fn a_bad_hash_is_moved_and_a_good_package_stays() {
        let lib = b"not a real library";
        let sha = sha256_bytes(lib);
        let manifest = format!(
            r#"[meta]
name = "example"
version = "1.2.3"
license = "MIT"

[install]
libraries = [
  {{ name = "example", path = "lib/libexample.so", sha256 = "{sha}" }}
]
"#
        );
        let good = tar_gz(&[
            ("package.toml", manifest.as_bytes()),
            ("LICENSE", b"MIT"),
            ("lib/libexample.so", lib),
        ]);
        let tmp = std::env::temp_dir().join(format!("acp-host-gate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let name = format!("example-{}-{}-1.2.3.tar.gz", host_os(), host_arch());
        let archive = tmp.join(&name);
        fs::write(&archive, &good).unwrap();
        let quarantine = tmp.join("quarantine");
        let got = gate_or_quarantine(&archive, &quarantine).unwrap();
        assert_eq!(got.name, "example");
        assert!(archive.is_file(), "a passing package stays in place");
        assert!(!quarantine.join(&name).exists());

        let bad_manifest = manifest.replace(&sha, &"ab".repeat(32));
        let bad = tar_gz(&[
            ("package.toml", bad_manifest.as_bytes()),
            ("LICENSE", b"MIT"),
            ("lib/libexample.so", lib),
        ]);
        fs::write(&archive, &bad).unwrap();
        let err = gate_or_quarantine(&archive, &quarantine).unwrap_err();
        assert!(err.contains("sha256 mismatch"), "{err}");
        assert!(quarantine.join(&name).is_file());
        assert!(!archive.exists());
        let reason = fs::read_to_string(quarantine.join(format!("{name}.reason"))).unwrap();
        assert!(reason.contains("sha256 mismatch"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn reload_leaves_a_good_package_and_quarantines_a_bad_one() {
        let lib = b"not a real library";
        let sha = sha256_bytes(lib);
        let manifest = format!(
            r#"[meta]
name = "example"
version = "1.2.3"
license = "MIT"

[install]
libraries = [
  {{ name = "example", path = "lib/libexample.so", sha256 = "{sha}" }}
]
"#
        );
        let good = tar_gz(&[
            ("package.toml", manifest.as_bytes()),
            ("LICENSE", b"MIT"),
            ("lib/libexample.so", lib),
        ]);
        let bad_manifest = manifest.replace(&sha, &"ab".repeat(32));
        let bad = tar_gz(&[
            ("package.toml", bad_manifest.as_bytes()),
            ("LICENSE", b"MIT"),
            ("lib/libexample.so", lib),
        ]);
        let tmp = std::env::temp_dir().join(format!("acp-reload-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let plugins = tmp.join("plugins");
        fs::create_dir_all(&plugins).unwrap();
        let good_name = format!("example-{}-{}-1.2.3.tar.gz", host_os(), host_arch());
        let bad_name = format!("other-{}-{}-1.2.3.tar.gz", host_os(), host_arch());
        fs::write(plugins.join(&good_name), &good).unwrap();
        fs::write(plugins.join(&bad_name), &bad).unwrap();
        let quarantine = tmp.join("quarantine");
        let missing = reload(&tmp.join("absent"), &quarantine);
        assert!(missing.passed.is_empty() && missing.quarantined.is_empty());
        let report = reload(&plugins, &quarantine);
        assert_eq!(report.passed, vec![good_name.clone()]);
        assert_eq!(report.quarantined, vec![bad_name.clone()]);
        assert!(plugins.join(&good_name).is_file());
        assert!(quarantine.join(&bad_name).is_file());
        assert!(!plugins.join(&bad_name).exists());
        let _ = fs::remove_dir_all(&tmp);
    }
}
