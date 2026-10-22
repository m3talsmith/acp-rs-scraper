//! Load plugins that passed the gate, and reload them.
//!
//! A changed archive is checked again. A gate failure drains in-flight calls,
//! unloads the library, and quarantines the archive. A version mismatch keeps
//! the registration already in memory and does not quarantine.

use super::gate::{self, sha256_bytes, GateFailure, Verified};
use super::quarantine;
use acp_red_star_node_core::service::{versions_match, CapabilityId, DynService, PluginRegistrar, RESERVED_IDS};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// One library the opener returned. `keep` holds the mapped file so drop does not unload it early.
pub struct Opened {
    /// rustc the plugin was built with.
    pub rustc_version: String,
    /// Core crate version the plugin was built against.
    pub core_version: String,
    /// Services it registered.
    pub services: Vec<Box<dyn DynService>>,
    /// Mapped library, when this came from `dlopen`.
    pub keep: Option<Arc<libloading::Library>>,
}

/// Opens the library inside a verified package.
pub trait OpenPlugin: Send + Sync {
    /// Read `plugin_declaration` from `library` and register its services.
    fn open(&self, library: &Path) -> Result<Opened, String>;
}

/// What reload did.
#[derive(Debug, Default)]
pub struct ReloadReport {
    /// Archives that stayed mapped.
    pub kept: usize,
    /// Archives newly registered.
    pub loaded: usize,
    /// Archives refused for a version mismatch. Not quarantined.
    pub refused: Vec<String>,
    /// Archives moved to quarantine.
    pub quarantined: Vec<String>,
}

/// A plugin that is loaded or refused.
pub struct Loaded {
    /// Archive filename.
    pub filename: String,
    /// sha256 of the archive bytes.
    pub digest: String,
    /// Services, when the version check passed.
    pub services: Vec<Box<dyn DynService>>,
    /// Why a new package was refused, when the old one is still mapped or nothing is mapped.
    pub refused: Option<String>,
    /// Keeps the library mapped.
    _keep: Vec<Arc<libloading::Library>>,
}

struct Registrar {
    services: Vec<Box<dyn DynService>>,
    notes: Vec<String>,
}

impl PluginRegistrar for Registrar {
    fn register_service(&mut self, service: Box<dyn DynService>) {
        let id = service.id();
        if !service.has(CapabilityId::Notification) {
            self.notes.push(format!("{id} does not implement notification"));
            return;
        }
        if RESERVED_IDS.contains(&id) || self.services.iter().any(|s| s.id() == id) {
            self.notes.push(format!("{id} is already registered"));
            return;
        }
        self.services.push(service);
    }
}

/// In-memory plugin host. The process keeps one of these.
pub struct Host {
    slots: Vec<Loaded>,
    busy: Arc<AtomicUsize>,
}

impl Host {
    /// Empty host.
    pub fn new() -> Self {
        Self { slots: Vec::new(), busy: Arc::new(AtomicUsize::new(0)) }
    }

    /// Call while a plugin method is on the stack. Reload waits until this is dropped.
    ///
    /// Nothing in this version calls into a loaded plugin yet, so production
    /// code does not take a guard. The drain still honours one.
    #[allow(dead_code)]
    pub fn enter(&self) -> BusyGuard {
        self.busy.fetch_add(1, Ordering::SeqCst);
        BusyGuard { busy: Arc::clone(&self.busy) }
    }

    /// Services from plugins that loaded.
    #[allow(dead_code)]
    pub fn services(&self) -> Vec<&dyn DynService> {
        self.slots.iter().flat_map(|s| s.services.iter().map(|p| p.as_ref())).collect()
    }

    /// Scan `~/.acp/plugins` and apply the gate.
    pub fn reload(&mut self, home: &Path, opener: &dyn OpenPlugin) -> ReloadReport {
        let mut report = ReloadReport::default();
        let dir = home.join(".acp/plugins");
        let archives = archives_in(&dir);
        let mut next = Vec::new();
        for archive in &archives {
            let filename = archive.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let bytes = match fs::read(archive) {
                Ok(b) => b,
                Err(e) => {
                    self.fail(home, archive, &filename, &format!("reading archive: {e}"), &mut report);
                    continue;
                }
            };
            let digest = sha256_bytes(&bytes);
            if let Some(pos) = self.slots.iter().position(|s| s.filename == filename && s.digest == digest && s.refused.is_none()) {
                next.push(self.slots.swap_remove(pos));
                report.kept += 1;
                continue;
            }
            let previous = self.slots.iter().position(|s| s.filename == filename);
            match open_one(archive, opener) {
                OpenOne::Loaded(opened) => {
                    if let Some(pos) = previous {
                        self.drain();
                        self.slots.swap_remove(pos);
                    }
                    let keep = opened.keep.into_iter().collect();
                    next.push(Loaded {
                        filename,
                        digest,
                        services: opened.services,
                        refused: None,
                        _keep: keep,
                    });
                    report.loaded += 1;
                }
                OpenOne::Mismatch { reason } => {
                    report.refused.push(format!("{filename}: {reason}"));
                    if let Some(pos) = previous {
                        let mut old = self.slots.swap_remove(pos);
                        old.refused = Some(reason);
                        next.push(old);
                    } else {
                        next.push(Loaded {
                            filename,
                            digest,
                            services: Vec::new(),
                            refused: Some(reason),
                            _keep: Vec::new(),
                        });
                    }
                }
                OpenOne::Failed(failure) => {
                    if let Some(pos) = previous {
                        self.drain();
                        self.slots.swap_remove(pos);
                    }
                    if let Ok(item) = quarantine::quarantine(home, "plugins", archive, &failure) {
                        report.quarantined.push(item.filename);
                    }
                }
            }
        }
        self.slots = next;
        report
    }

    fn fail(&mut self, home: &Path, archive: &Path, filename: &str, reason: &str, report: &mut ReloadReport) {
        if let Some(pos) = self.slots.iter().position(|s| s.filename == filename) {
            self.drain();
            self.slots.swap_remove(pos);
        }
        let failure = GateFailure { name: filename.into(), reason: reason.into() };
        if let Ok(item) = quarantine::quarantine(home, "plugins", archive, &failure) {
            report.quarantined.push(item.filename);
        }
    }

    fn drain(&self) {
        while self.busy.load(Ordering::SeqCst) > 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

impl Default for Host {
    fn default() -> Self { Self::new() }
}

/// A loaded or refused plugin, for the Services screen and `--services`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// Archive filename.
    pub filename: String,
    /// Service id, empty when the package was refused before register.
    pub id: String,
    /// Display name.
    pub display: String,
    /// Version mismatch, when the package was refused.
    pub refused: Option<String>,
}

impl Host {
    /// Loaded and refused plugins.
    pub fn summaries(&self) -> Vec<Summary> {
        self.slots.iter().map(|slot| {
            if let Some(service) = slot.services.first() {
                Summary {
                    filename: slot.filename.clone(),
                    id: service.id().into(),
                    display: service.display_name().into(),
                    refused: slot.refused.clone(),
                }
            } else {
                Summary {
                    filename: slot.filename.clone(),
                    id: String::new(),
                    display: String::new(),
                    refused: slot.refused.clone(),
                }
            }
        }).collect()
    }
}

/// Decrements the in-flight count.
pub struct BusyGuard {
    busy: Arc<AtomicUsize>,
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.busy.fetch_sub(1, Ordering::SeqCst);
    }
}

enum OpenOne {
    Loaded(Opened),
    Mismatch { reason: String },
    Failed(GateFailure),
}

fn open_one(archive: &Path, opener: &dyn OpenPlugin) -> OpenOne {
    let temp = std::env::temp_dir().join(format!(
        "acp-plugin-{}-{}",
        std::process::id(),
        sha256_bytes(archive.to_string_lossy().as_bytes()).chars().take(8).collect::<String>()
    ));
    let _ = fs::remove_dir_all(&temp);
    let verified = match gate::verify_archive(archive, &temp) {
        Ok(v) => v,
        Err(failure) => {
            let _ = fs::remove_dir_all(&temp);
            return OpenOne::Failed(failure);
        }
    };
    let opened = match opener.open(&verified.library) {
        Ok(v) => v,
        Err(reason) => {
            let _ = fs::remove_dir_all(&temp);
            return OpenOne::Failed(GateFailure { name: verified.name, reason });
        }
    };
    if let Err(reason) = versions_match(&opened.rustc_version, &opened.core_version) {
        let _ = fs::remove_dir_all(&temp);
        return OpenOne::Mismatch { reason };
    }
    let mut reg = Registrar { services: Vec::new(), notes: Vec::new() };
    for service in opened.services {
        reg.register_service(service);
    }
    if reg.services.is_empty() {
        let reason = reg.notes.pop().unwrap_or_else(|| "plugin registered no service".into());
        let _ = fs::remove_dir_all(&temp);
        return OpenOne::Failed(GateFailure { name: verified.name, reason });
    }
    keep_tree(archive, &verified, &temp);
    OpenOne::Loaded(Opened {
        rustc_version: opened.rustc_version,
        core_version: opened.core_version,
        services: reg.services,
        keep: opened.keep,
    })
}

fn keep_tree(archive: &Path, verified: &Verified, temp: &Path) {
    let dest = archive
        .parent()
        .unwrap_or(temp)
        .join(&verified.name)
        .join(&verified.version);
    let _ = fs::remove_dir_all(&dest);
    if fs::rename(temp, &dest).is_err() {
        let _ = fs::remove_dir_all(temp);
    }
}

fn archives_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| {
        p.file_name().and_then(|s| s.to_str()).is_some_and(|n| n.ends_with(".tar.gz"))
    }).collect();
    v.sort();
    v
}

fn host_lock() -> &'static Mutex<Host> {
    static HOST: std::sync::OnceLock<Mutex<Host>> = std::sync::OnceLock::new();
    HOST.get_or_init(|| Mutex::new(Host::new()))
}

/// Reload plugins under `home` with the real `dlopen` opener.
pub fn reload(home: &Path) -> ReloadReport {
    let mut host = host_lock().lock().unwrap_or_else(|e| e.into_inner());
    host.reload(home, &Dylib)
}

/// First scan of this process. A missing plugins directory does nothing.
pub fn startup(home: &Path) -> ReloadReport {
    reload(home)
}

/// Plugins currently loaded or refused.
pub fn summaries() -> Vec<Summary> {
    host_lock().lock().unwrap_or_else(|e| e.into_inner()).summaries()
}

struct Dylib;

impl OpenPlugin for Dylib {
    fn open(&self, library: &Path) -> Result<Opened, String> {
        let lib = unsafe { libloading::Library::new(library) }.map_err(|e| format!("dlopen failed: {e}"))?;
        let (rustc_version, core_version, register) = {
            let sym = unsafe {
                lib.get::<acp_red_star_node_core::service::PluginDeclaration>(b"plugin_declaration\0")
                    .map_err(|e| format!("plugin_declaration is missing: {e}"))?
            };
            (sym.rustc_version.to_string(), sym.core_version.to_string(), sym.register)
        };
        let mut reg = Registrar { services: Vec::new(), notes: Vec::new() };
        unsafe { register(&mut reg) };
        let services = reg.services;
        let mut notes = reg.notes;
        if services.is_empty() {
            return Err(notes.pop().unwrap_or_else(|| "plugin registered no service".into()));
        }
        Ok(Opened { rustc_version, core_version, services, keep: Some(Arc::new(lib)) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::gate::{host_arch, host_os, sha256_bytes};
    use acp_red_star_node_core::service::{Availability, Field, RuntimeCtx, CORE_VERSION, RUSTC_VERSION};
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::sync::atomic::AtomicBool;

    #[derive(Default)]
    struct Example;

    impl DynService for Example {
        fn id(&self) -> &'static str { "example" }
        fn display_name(&self) -> &'static str { "Example" }
        fn implemented(&self) -> &'static [CapabilityId] { &[CapabilityId::Notification, CapabilityId::Reader] }
        fn schema(&self) -> &'static [Field] { &[] }
        fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
            if ctx.enabled && self.has(cap) { Availability::Ready } else { Availability::Off }
        }
    }

    struct Script {
        mode: Mutex<Mode>,
    }

    enum Mode {
        Match,
        Mismatch,
        Boom,
    }

    impl OpenPlugin for Script {
        fn open(&self, _library: &Path) -> Result<Opened, String> {
            match *self.mode.lock().unwrap() {
                Mode::Boom => Err("library will not open".into()),
                Mode::Mismatch => Ok(Opened {
                    rustc_version: "rustc 0.0.0".into(),
                    core_version: CORE_VERSION.into(),
                    services: vec![Box::new(Example)],
                    keep: None,
                }),
                Mode::Match => Ok(Opened {
                    rustc_version: RUSTC_VERSION.into(),
                    core_version: CORE_VERSION.into(),
                    services: vec![Box::new(Example)],
                    keep: None,
                }),
            }
        }
    }

    fn archive(dir: &Path, lib: &[u8]) -> PathBuf {
        let sha = sha256_bytes(lib);
        let manifest = format!(
            "[meta]\nname = \"example\"\nversion = \"1.2.3\"\n\n[install]\nlibraries = [\n  {{ name = \"example\", path = \"lib/libexample.so\", sha256 = \"{sha}\" }}\n]\n"
        );
        let mut body = Vec::new();
        {
            let enc = GzEncoder::new(&mut body, Compression::default());
            let mut builder = tar::Builder::new(enc);
            for (path, data) in [("package.toml", manifest.as_bytes()), ("lib/libexample.so", lib)] {
                let mut header = tar::Header::new_gnu();
                header.set_path(path).unwrap();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append(&header, data).unwrap();
            }
            builder.finish().unwrap();
        }
        let plugins = dir.join(".acp/plugins");
        fs::create_dir_all(&plugins).unwrap();
        let path = plugins.join(format!("example-{}-{}-1.2.3.tar.gz", host_os(), host_arch()));
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn a_good_package_loads_a_mismatch_stays_and_a_bad_change_is_quarantined_after_drain() {
        let home = std::env::temp_dir().join(format!("acp-plug-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let _ = fs::remove_dir_all(home.join(".acp/quarantined"));
        archive(&home, b"lib-v1");
        let opener = Script { mode: Mutex::new(Mode::Match) };
        let mut host = Host::new();
        let report = host.reload(&home, &opener);
        assert_eq!(report.loaded, 1, "refused {:?}", report.refused);
        assert!(report.quarantined.is_empty());
        assert_eq!(host.services().len(), 1);
        assert!(quarantine::list(&home).is_empty());

        *opener.mode.lock().unwrap() = Mode::Mismatch;
        archive(&home, b"lib-v2");
        let report = host.reload(&home, &opener);
        assert_eq!(report.refused.len(), 1, "{:?}", report.refused);
        assert!(report.quarantined.is_empty());
        assert_eq!(host.services().len(), 1, "the loaded plugin stays");
        assert!(quarantine::list(&home).is_empty());

        *opener.mode.lock().unwrap() = Mode::Boom;
        archive(&home, b"lib-v3");
        let flag = Arc::new(AtomicBool::new(false));
        let guard = host.enter();
        let flag2 = Arc::clone(&flag);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            flag2.store(true, Ordering::SeqCst);
            drop(guard);
        });
        let report = host.reload(&home, &opener);
        assert!(flag.load(Ordering::SeqCst), "unload waited until the call finished");
        assert_eq!(report.quarantined.len(), 1, "{:?}", report.quarantined);
        assert!(host.services().is_empty());
        let items = quarantine::list(&home);
        assert_eq!(items.len(), 1);
        assert!(items[0].reason.contains("potential security threat"), "{}", items[0].reason);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn versions_match_is_the_only_pin_check_here() {
        assert!(versions_match(RUSTC_VERSION, CORE_VERSION).is_ok());
    }
}
