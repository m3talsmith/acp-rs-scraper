use crate::tui::jobs::*;
use std::time::{Duration, Instant};

fn log(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("acp-tui-jobs-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d.join("job.log")
}

fn wait(j: &mut Job) -> (Vec<String>, std::process::ExitStatus) {
    let t = Instant::now();
    let mut all = Vec::new();
    loop {
        let (l, st) = j.poll();
        all.extend(l);
        if let Some(st) = st { all.extend(j.poll().0); return (all, st); }
        assert!(t.elapsed() < Duration::from_secs(10), "job hung");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn job_output_goes_to_a_private_log_and_status_is_reported() {
    let p = log("out");
    let argv: Vec<String> = ["x", "-c", "echo one >&2; echo two >&2; echo stdout-is-dropped; exit 3"].map(String::from).to_vec();
    let mut j = Job::spawn_with(std::path::Path::new("/bin/sh"), JobKind::Scan, &argv, p.clone()).unwrap();
    let (lines, st) = wait(&mut j);
    assert_eq!(lines, ["one", "two"]);
    assert_eq!(st.code(), Some(3));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(tail(&p, 1), ["two"]);
}

#[cfg(unix)]
#[test]
fn job_runs_in_its_own_process_group_and_cancel_kills_it() {
    let p = log("cancel");
    let argv: Vec<String> = ["x", "-c", "ps -o pgid= -p $$ >&2; sleep 30"].map(String::from).to_vec();
    let mut j = Job::spawn_with(std::path::Path::new("/bin/sh"), JobKind::Scan, &argv, p).unwrap();
    let pid = j.pid();
    let t = Instant::now();
    let mut lines = Vec::new();
    while lines.is_empty() && t.elapsed() < Duration::from_secs(5) { lines = j.poll().0; std::thread::sleep(Duration::from_millis(20)); }
    if let Some(pg) = lines.first() { assert_eq!(pg.trim(), pid.to_string(), "own process group (pgid == pid)"); }
    let t = Instant::now();
    j.cancel().unwrap();
    assert!(t.elapsed() < Duration::from_secs(5));
    assert!(j.poll().1.is_some());
}

#[test]
fn job_kind_names() {
    assert_eq!([JobKind::Scan, JobKind::Export, JobKind::Import, JobKind::Maintain].map(JobKind::name), ["scan", "export", "import", "maintain"]);
}
