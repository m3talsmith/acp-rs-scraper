//! Host side of the package gate and the loader.
//!
//! A failed archive moves to the quarantine directory and is never opened.
//! A rustc ABI or plugin-crate mismatch is refused by [`crate::service::admit`]
//! and is not moved. A package that passes is opened with `libloading`.
//! `plugins.reload` is not called on startup and it is not bound to a key.
//! A command that needs a service loads the packages that are already installed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use acp_rs_plugin::{verify_archive, CapabilityId, DynService, PluginDeclaration, Verified};

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
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReloadReport {
    /// Filenames that passed the archive gate and stayed in place.
    pub passed: Vec<String>,
    /// Filenames moved into the quarantine directory. These were not opened.
    pub quarantined: Vec<String>,
    /// `package capability` lines for libraries that registered.
    pub loaded: Vec<String>,
    /// Gate passed, but the library was refused. The archive stays put.
    pub refused: Vec<String>,
}

struct Loaded {
    modules: Vec<Arc<PluginModule>>,
    scanned: bool,
}

/// One opened library. Services are dropped before the library is unmapped.
struct PluginModule {
    name: String,
    services: Vec<Box<dyn DynService>>,
    /// Kept so `Drop` unmaps the library after the services are dropped.
    #[allow(dead_code)]
    library: LibKeep,
}

/// `Library` is `Send` and not `Sync`. After open, the only use is `Drop`.
#[allow(dead_code)]
struct LibKeep(libloading::Library);

// SAFETY: nothing reads the library after `open`. `Drop` runs when the last
// `Arc<PluginModule>` is gone, which is after that call has released the services.
unsafe impl Sync for LibKeep {}

static STATE: Mutex<Loaded> = Mutex::new(Loaded {
    modules: Vec::new(),
    scanned: false,
});
static LOAD: Mutex<()> = Mutex::new(());

#[cfg(test)]
std::thread_local! {
    static LOADER_TEST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn consider_plugins() -> bool {
    #[cfg(test)]
    {
        LOADER_TEST.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

/// A built-in, or a plugin service kept alive by its library.
pub struct Live {
    kind: LiveKind,
}

enum LiveKind {
    Builtin(usize),
    Plugin { module: Arc<PluginModule>, index: usize },
}

impl Live {
    /// The service. A plugin library stays mapped while this value exists.
    pub fn get(&self) -> &dyn DynService {
        match &self.kind {
            LiveKind::Builtin(index) => crate::service::runtime().services()[*index].as_ref(),
            LiveKind::Plugin { module, index } => module.services[*index].as_ref(),
        }
    }
}

/// Load installed packages once, then return the service that implements `cap`.
///
/// An installed package is preferred over a built-in. Under `cfg(test)` this
/// does not scan a home directory unless `ACP_HOME` is set, and it does not
/// see a library a loader test opened unless that test opted in.
pub fn live(cap: CapabilityId) -> Option<Live> {
    ensure_loaded();
    if consider_plugins()
        && let Some(found) = plugin_live(cap)
    {
        return Some(found);
    }
    builtin_live(cap)
}

fn plugin_live(cap: CapabilityId) -> Option<Live> {
    let state = lock(&STATE);
    for module in &state.modules {
        if let Some(index) = module.services.iter().position(|service| service.has(cap)) {
            return Some(Live {
                kind: LiveKind::Plugin {
                    module: Arc::clone(module),
                    index,
                },
            });
        }
    }
    None
}

fn builtin_live(cap: CapabilityId) -> Option<Live> {
    crate::service::runtime()
        .services()
        .iter()
        .position(|service| service.has(cap))
        .map(|index| Live {
            kind: LiveKind::Builtin(index),
        })
}

/// Scan `~/.acp/plugins` once per process. The quarantine directory is not scanned.
pub fn ensure_loaded() {
    if cfg!(test) && std::env::var_os("ACP_HOME").filter(|value| !value.is_empty()).is_none() {
        return;
    }
    let _guard = lock(&LOAD);
    if lock(&STATE).scanned {
        return;
    }
    let Ok(home) = crate::config::home() else {
        lock(&STATE).scanned = true;
        return;
    };
    scan(&home.join(".acp/plugins"), &home.join(".acp/quarantined/plugins"));
}

/// Unload loaded packages and scan `plugins_dir` again.
///
/// This is what `plugins.reload` calls. A missing directory is an empty report.
/// A failed archive is logged as a potential security threat and moved into
/// `quarantine_dir`. A rustc ABI or plugin-crate mismatch stays in place.
/// This function does not run on startup, and it is not bound to a key.
pub fn reload(plugins_dir: &Path, quarantine_dir: &Path) -> ReloadReport {
    let _guard = lock(&LOAD);
    {
        let mut state = lock(&STATE);
        state.modules.clear();
        state.scanned = false;
    }
    scan(plugins_dir, quarantine_dir)
}

/// `--plugins-reload`: print what loaded, and fail when anything was quarantined or refused.
pub fn run_reload() -> anyhow::Result<()> {
    let home = crate::config::home()?;
    let quarantine = home.join(".acp/quarantined/plugins");
    let report = reload(&home.join(".acp/plugins"), &quarantine);
    for line in &report.loaded {
        println!("loaded {line}");
    }
    let mut errors = Vec::new();
    errors.extend(report.refused.iter().cloned());
    for name in &report.quarantined {
        let reason = fs::read_to_string(quarantine.join(format!("{name}.reason")))
            .unwrap_or_else(|_| "quarantined".into());
        errors.push(format!("{name}: {reason}"));
    }
    if !errors.is_empty() {
        anyhow::bail!("{}", errors.join("\n"));
    }
    Ok(())
}

fn scan(plugins_dir: &Path, quarantine_dir: &Path) -> ReloadReport {
    let mut report = ReloadReport::default();
    let archives = match list_archives(plugins_dir) {
        Ok(archives) => archives,
        Err(err) => {
            crate::logging::log_error(format_args!(
                "potential security threat: reading {}: {err}",
                plugins_dir.display()
            ));
            lock(&STATE).scanned = true;
            return report;
        }
    };
    let mut modules = Vec::new();
    for archive in archives {
        let name = archive
            .file_name()
            .and_then(|item| item.to_str())
            .unwrap_or("plugin.tar.gz")
            .to_string();
        match gate_or_quarantine(&archive, quarantine_dir) {
            Ok(verified) => {
                report.passed.push(name.clone());
                match open_verified(&verified, quarantine_dir, &modules) {
                    Ok(module) => {
                        report.loaded.push(loaded_line(&module));
                        modules.push(module);
                    }
                    Err(reason) => {
                        crate::logging::log_error(format_args!("plugin {name} refused: {reason}"));
                        report.refused.push(format!("{name}: {reason}"));
                    }
                }
            }
            Err(reason) => {
                crate::logging::log_error(format_args!(
                    "potential security threat: plugin {name}: {reason}"
                ));
                report.quarantined.push(name);
            }
        }
    }
    let mut state = lock(&STATE);
    state.modules = modules.into_iter().map(Arc::new).collect();
    state.scanned = true;
    report
}

fn list_archives(plugins_dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let entries = match fs::read_dir(plugins_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
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
    Ok(archives)
}

fn loaded_line(module: &PluginModule) -> String {
    let mut caps = Vec::new();
    for service in &module.services {
        for cap in service.implemented() {
            let name = cap.as_str();
            if !caps.contains(&name) {
                caps.push(name);
            }
        }
    }
    format!("{} {}", module.name, caps.join(" "))
}

/// Open `verified.library` only. A path that is not inside the unpacked tree, or that is inside quarantine, is refused.
fn library_to_open(verified: &Verified, quarantine_dir: &Path) -> Result<PathBuf, String> {
    let library = verified
        .library
        .canonicalize()
        .map_err(|err| format!("refusing an unverified library: {err}"))?;
    let root = verified
        .root
        .canonicalize()
        .map_err(|err| format!("refusing an unverified library: {err}"))?;
    if !library.starts_with(&root) {
        return Err("refusing a library outside the verified package".into());
    }
    if let Ok(quarantine) = quarantine_dir.canonicalize()
        && library.starts_with(&quarantine)
    {
        return Err("refusing a quarantined library".into());
    }
    Ok(library)
}

fn open_verified(
    verified: &Verified,
    quarantine_dir: &Path,
    loaded: &[PluginModule],
) -> Result<PluginModule, String> {
    let path = library_to_open(verified, quarantine_dir)?;
    let library = unsafe { libloading::Library::new(&path) }
        .map_err(|err| format!("could not open the library: {err}"))?;
    let services = read_and_register(&library, verified, loaded)?;
    Ok(PluginModule {
        name: verified.name.clone(),
        services,
        library: LibKeep(library),
    })
}

fn read_and_register(
    library: &libloading::Library,
    verified: &Verified,
    loaded: &[PluginModule],
) -> Result<Vec<Box<dyn DynService>>, String> {
    // `Library::get` only accepts a pointer-sized type. The symbol address is the
    // static, so the pointer is `*const PluginDeclaration` and one more read copies it.
    let (rustc_version, core_version, register) = {
        let symbol = unsafe { library.get::<*const PluginDeclaration>(b"plugin_declaration\0") }
            .map_err(|err| format!("plugin_declaration: {err}"))?;
        let declaration = unsafe { **symbol };
        (declaration.rustc_version, declaration.core_version, declaration.register)
    };
    crate::service::admit(rustc_version, core_version)?;
    let mut reg = crate::service::Registry::external(verified.official);
    unsafe { register(&mut reg) };
    let services = reg.into_services();
    if services.is_empty() {
        return Err("no service was registered".into());
    }
    for service in &services {
        let id = service.id();
        let taken = loaded
            .iter()
            .any(|module| module.services.iter().any(|other| other.id() == id))
            || crate::service::runtime()
                .services()
                .iter()
                .any(|other| other.id() == id);
        if taken {
            return Err(format!("service id {id} is already loaded"));
        }
    }
    Ok(services)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_rs_plugin::{host_arch, host_os, sha256_bytes, Budget, Cursor, Target};
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::sync::Mutex as TestMutex;

    static LOADER_TESTS: TestMutex<()> = TestMutex::new(());

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

    fn shared_library(crate_name: &str) -> PathBuf {
        let mut path = std::env::current_exe().unwrap();
        // Unit tests run from target/debug/deps, next to the fixture cdylibs.
        path.pop();
        path.join(format!(
            "{}{}{}",
            std::env::consts::DLL_PREFIX,
            crate_name,
            std::env::consts::DLL_SUFFIX
        ))
    }

    fn write_package(dir: &Path, so: &[u8], name: &str, version: &str, sha: &str) -> String {
        let manifest = format!(
            r#"[meta]
name = "{name}"
version = "{version}"
license = "MIT"

[install]
libraries = [
  {{ name = "{name}", path = "lib/libfixture.so", sha256 = "{sha}" }}
]
"#
        );
        let bytes = tar_gz(&[
            ("package.toml", manifest.as_bytes()),
            ("LICENSE", b"MIT"),
            ("lib/libfixture.so", so),
        ]);
        let filename = format!("{name}-{}-{}-{version}.tar.gz", host_os(), host_arch());
        fs::write(dir.join(&filename), bytes).unwrap();
        filename
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
        let _guard = LOADER_TESTS.lock().unwrap_or_else(|err| err.into_inner());
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
        assert!(missing.loaded.is_empty() && missing.refused.is_empty());
        let report = reload(&plugins, &quarantine);
        assert_eq!(report.passed, vec![good_name.clone()]);
        assert_eq!(report.quarantined, vec![bad_name.clone()]);
        assert!(report.loaded.is_empty(), "{:?}", report.loaded);
        assert!(
            report.refused.iter().any(|line| line.contains(&good_name)),
            "{:?}",
            report.refused
        );
        assert!(
            report.refused.iter().all(|line| !line.contains(&bad_name)),
            "a quarantined archive was opened: {:?}",
            report.refused
        );
        assert!(plugins.join(&good_name).is_file());
        assert!(quarantine.join(&bad_name).is_file());
        assert!(!plugins.join(&bad_name).exists());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn a_library_inside_quarantine_is_refused_before_it_is_opened() {
        let tmp = std::env::temp_dir().join(format!("acp-quarantine-path-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let quarantine = tmp.join("quarantine");
        fs::create_dir_all(&quarantine).unwrap();
        let library = quarantine.join("libexample.so");
        fs::write(&library, b"not opened").unwrap();
        let verified = Verified {
            name: "example".into(),
            version: "1.2.3".into(),
            root: quarantine.clone(),
            library: library.clone(),
            official: false,
        };
        let err = library_to_open(&verified, &quarantine).unwrap_err();
        assert!(err.contains("quarantined"), "{err}");
        let missing = Verified {
            library: quarantine.join("missing.so"),
            ..verified
        };
        let err = library_to_open(&missing, &quarantine.join("other")).unwrap_err();
        assert!(err.contains("unverified"), "{err}");
        assert_eq!(fs::read(&library).unwrap(), b"not opened");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn a_verified_fixture_is_called_reloaded_and_a_mismatch_is_refused() {
        let _guard = LOADER_TESTS.lock().unwrap_or_else(|err| err.into_inner());
        LOADER_TEST.with(|flag| flag.set(true));
        let _clear = ClearLoader;
        let tmp = std::env::temp_dir().join(format!("acp-fixture-load-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let plugins = tmp.join("plugins");
        let quarantine = tmp.join("quarantine");
        fs::create_dir_all(&plugins).unwrap();
        let so = fs::read(shared_library("acp_rs_fixture")).expect("fixture cdylib");
        let sha = sha256_bytes(&so);
        write_package(&plugins, &so, "fixture", "0.1.0", &sha);
        let report = reload(&plugins, &quarantine);
        assert!(report.quarantined.is_empty(), "{:?}", report.quarantined);
        assert!(report.refused.is_empty(), "{:?}", report.refused);
        assert_eq!(report.loaded, vec!["fixture reader".to_string()]);
        let body = page_body();
        assert_eq!(body, "from the fixture");

        let next = fs::read(shared_library("acp_rs_fixture_next")).expect("reloaded fixture");
        let next_sha = sha256_bytes(&next);
        write_package(&plugins, &next, "fixture", "0.1.0", &next_sha);
        let report = reload(&plugins, &quarantine);
        assert!(report.refused.is_empty(), "{:?}", report.refused);
        assert_eq!(report.loaded, vec!["fixture reader".to_string()]);
        assert_eq!(page_body(), "from the reloaded fixture");

        let _ = fs::remove_dir_all(&plugins);
        fs::create_dir_all(&plugins).unwrap();
        let mismatch = fs::read(shared_library("acp_rs_fixture_mismatch")).expect("mismatch cdylib");
        let mismatch_sha = sha256_bytes(&mismatch);
        let filename = write_package(&plugins, &mismatch, "mismatch", "0.1.0", &mismatch_sha);
        let report = reload(&plugins, &quarantine);
        assert!(report.loaded.is_empty(), "{:?}", report.loaded);
        assert!(report.quarantined.is_empty(), "{:?}", report.quarantined);
        assert!(
            report.refused.iter().any(|line| line.contains("core mismatch")),
            "{:?}",
            report.refused
        );
        assert!(plugins.join(&filename).is_file(), "a version mismatch stays in place");
        assert!(live(CapabilityId::Reader).is_none());

        let _ = fs::remove_dir_all(&tmp);
        let _ = reload(&tmp.join("absent"), &quarantine);
    }

    struct ClearLoader;

    impl Drop for ClearLoader {
        fn drop(&mut self) {
            LOADER_TEST.with(|flag| flag.set(false));
            lock(&STATE).modules.clear();
        }
    }

    fn page_body() -> String {
        let live = live(CapabilityId::Reader).expect("reader");
        assert_eq!(live.get().id(), "fixture");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let resolved = live
                .get()
                .resolve(&Target::Account { handle: "ada".into() })
                .await
                .unwrap();
            let mut budget = Budget::unlimited();
            let page = live
                .get()
                .page(&resolved, &Cursor::default(), None, &mut budget)
                .await
                .unwrap();
            assert!(page.next.is_none());
            page.items[0].body.clone()
        })
    }
}
