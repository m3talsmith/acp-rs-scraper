//! LiveView uploads for the site copy of a new post or a reply.
//!
//! The frames match a redacted capture of one new post (2026-10-21): one
//! `allow_upload` on the page channel, an `lvu:<entry ref>` join per file,
//! binary `chunk` frames, then `progress` on the page channel. Save stays the
//! text frame and carries no `uploads` key. The write-up is
//! `architecture/UPLOAD-CAPTURE.md`. The synthetic fixture is
//! `tests/fixtures/site/upload.json`.

#[cfg(test)]
use super::attach::AttachmentKind;
use super::attach::PlannedAttachment;
use super::write::WritePlan;
use crate::prelude::*;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// Chunk size from the captured `allow_upload` reply. Dry-run uses this. A live reply can replace it.
pub const CAPTURED_CHUNK_SIZE: u64 = 64_000;
/// `max_file_size` from that reply.
pub const CAPTURED_MAX_FILE_SIZE: u64 = 250_000_000;
/// `max_entries` from that reply.
pub const CAPTURED_MAX_ENTRIES: u64 = 4;
/// `chunk_timeout` from that reply, in milliseconds.
pub const CAPTURED_CHUNK_TIMEOUT_MS: u64 = 10_000;
/// Upload ref printed by dry-run. The live ref is read from the joined page.
pub const DRY_RUN_UPLOAD_REF: &str = "phx:<page>";
/// Largest chunk this client will allocate. A larger server value stops the upload.
const MAX_CHUNK_ALLOC: u64 = 8_000_000;

/// One file the `allow_upload` preflight offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadEntry {
    /// File mtime in milliseconds, or 0 when the clock cannot be read.
    pub last_modified: u64,
    /// File name. `.jpeg` and `.tif` are not renamed.
    pub name: String,
    /// Always empty in the capture.
    pub relative_path: String,
    /// Size in bytes.
    pub size: u64,
    /// MIME type sent as `type`.
    pub mime: String,
    /// Entry ref (`"0"`, `"1"`, …). The server keys the join token by this.
    pub entry_ref: String,
}

/// `allow_upload` payload. This is a top-level channel event, not an `event` form.
pub fn allow_upload_payload(upload_ref: &str, cid: u64, entries: &[UploadEntry]) -> Value {
    json!({
        "ref": upload_ref,
        "entries": entries.iter().map(|e| json!({
            "last_modified": e.last_modified,
            "name": e.name,
            "relative_path": e.relative_path,
            "size": e.size,
            "type": e.mime,
            "ref": e.entry_ref,
        })).collect::<Vec<_>>(),
        "cid": cid,
    })
}

/// `progress` payload on the page channel. `event` is null. Save is a later frame.
pub fn progress_payload(upload_ref: &str, entry_ref: &str, progress: u64, cid: u64) -> Value {
    json!({
        "event": null,
        "ref": upload_ref,
        "entry_ref": entry_ref,
        "progress": progress,
        "cid": cid,
    })
}

/// The `lvu` join frame. `join_ref` and the message ref are the same string.
///
/// The live join is [`crate::liveview::Lv::join_topic`], which sends this same array.
#[cfg(test)]
pub fn lvu_join_message(join_ref: &str, topic: &str, token: &str) -> Value {
    json!([join_ref, join_ref, topic, "phx_join", {"token": token}])
}

/// Integer percent, `floor(sent * 100 / size)`, and 100 when `sent >= size`.
pub fn upload_percent(sent: u64, size: u64) -> u64 {
    if size == 0 || sent >= size {
        return 100;
    }
    sent.saturating_mul(100) / size
}

/// `(offset, len)` slices of `size` in steps of `chunk_size`.
pub fn chunk_spans(size: u64, chunk_size: u64) -> Vec<(u64, u64)> {
    if chunk_size == 0 || size == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut off = 0u64;
    while off < size {
        let n = chunk_size.min(size - off);
        out.push((off, n));
        off += n;
    }
    out
}

/// Limits from an `allow_upload` reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowConfig {
    /// Maximum bytes in one binary chunk.
    pub chunk_size: u64,
    /// How long to wait for one chunk reply, in milliseconds.
    pub chunk_timeout_ms: u64,
    /// How many entries this input accepts.
    pub max_entries: u64,
    /// How large one file may be.
    pub max_file_size: u64,
}

/// Read `config` and the per-entry tokens. A non-empty `errors` map, or a missing token, is a refusal.
///
/// The token strings are returned to the caller and are not included in the error text.
pub fn parse_allow_reply(reply: &Value, entry_refs: &[&str]) -> Result<(AllowConfig, BTreeMap<String, String>), String> {
    if reply.get("status").and_then(Value::as_str) != Some("ok") {
        let reason = reply.pointer("/response/reason").map(|v| v.to_string()).unwrap_or_else(|| "error".into());
        return Err(format!("the site rejected the upload ({}). Nothing was sent.", brief(&reason)));
    }
    let response = &reply["response"];
    let errors = &response["errors"];
    if errors_present(errors) {
        return Err(format!("the site rejected an upload ({}). Nothing was sent.", brief(&format_errors(errors))));
    }
    let cfg = &response["config"];
    let Some(chunk_size) = cfg["chunk_size"].as_u64().filter(|n| *n > 0) else {
        return Err("the site did not say how to chunk the upload. Nothing was sent.".into());
    };
    if chunk_size > MAX_CHUNK_ALLOC {
        return Err(format!(
            "the site asked for a chunk of {chunk_size} bytes, which this client will not allocate. Nothing was sent."
        ));
    }
    let config = AllowConfig {
        chunk_size,
        chunk_timeout_ms: cfg["chunk_timeout"].as_u64().filter(|n| *n > 0).unwrap_or(CAPTURED_CHUNK_TIMEOUT_MS),
        max_entries: cfg["max_entries"].as_u64().unwrap_or(CAPTURED_MAX_ENTRIES),
        max_file_size: cfg["max_file_size"].as_u64().filter(|n| *n > 0).unwrap_or(CAPTURED_MAX_FILE_SIZE),
    };
    let Some(entries) = response["entries"].as_object() else {
        return Err("the site did not return upload tokens. Nothing was sent.".into());
    };
    let mut tokens = BTreeMap::new();
    for r in entry_refs {
        match entries.get(*r).and_then(Value::as_str).filter(|t| !t.is_empty()) {
            Some(token) => {
                tokens.insert((*r).to_string(), token.to_string());
            }
            None => {
                return Err(format!("the site did not return an upload token for entry {r}. Nothing was sent."));
            }
        }
    }
    Ok((config, tokens))
}

fn errors_present(errors: &Value) -> bool {
    match errors {
        Value::Null => false,
        Value::Object(map) => !map.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::String(s) => !s.is_empty(),
        other => !other.is_null(),
    }
}

fn format_errors(errors: &Value) -> String {
    let Some(obj) = errors.as_object() else {
        return errors.to_string();
    };
    obj.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("; ")
}

fn brief(s: &str) -> String {
    let s = crate::logging::redact_secrets(s);
    let mut t: String = s.chars().take(300).collect();
    if t.chars().count() < s.chars().count() {
        t.push('…');
    }
    t
}

/// A file input from the joined page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadInput {
    /// `media_attachments` or `file_attachments`.
    pub name: String,
    /// `data-phx-upload-ref`.
    pub upload_ref: String,
    /// The `accept` attribute. Empty means the form did not say.
    pub accept: String,
}

/// File inputs that carry `data-phx-upload-ref`. A preview node with the same ref borrows the input's name.
pub fn upload_inputs(html: &str) -> Vec<UploadInput> {
    let doc = Html::parse_document(html);
    let Ok(sel) = Selector::parse("[data-phx-upload-ref]") else { return Vec::new() };
    let mut out: Vec<UploadInput> = Vec::new();
    for el in doc.select(&sel) {
        let Some(upload_ref) = el.value().attr("data-phx-upload-ref").filter(|s| !s.is_empty()) else { continue };
        let name = el.value().attr("name").unwrap_or("").to_string();
        let accept = el.value().attr("accept").unwrap_or("").to_string();
        if let Some(existing) = out.iter_mut().find(|i| i.upload_ref == upload_ref) {
            if existing.name.is_empty() && !name.is_empty() {
                existing.name = name;
            }
            if existing.accept.is_empty() && !accept.is_empty() {
                existing.accept = accept;
            }
            continue;
        }
        out.push(UploadInput { name, upload_ref: upload_ref.to_string(), accept });
    }
    if out.iter().any(|i| !i.name.is_empty()) {
        out.retain(|i| !i.name.is_empty());
    }
    out
}

/// `true` when `filename` matches an `accept` list. An empty list does not refuse the file.
///
/// `.tif` does not match `.tiff`. The file is not renamed.
pub fn accept_allows(accept: &str, filename: &str) -> bool {
    let accept = accept.trim();
    if accept.is_empty() || accept == "*/*" {
        return true;
    }
    let ext = extension_of(filename);
    let mime = mime_for_filename(filename);
    accept.split(',').any(|part| {
        let part = part.trim().to_ascii_lowercase();
        if part.is_empty() || part == "*/*" {
            return part == "*/*";
        }
        if let Some(e) = part.strip_prefix('.') {
            return e == ext;
        }
        if let Some(prefix) = part.strip_suffix('*') {
            return !prefix.is_empty() && mime.starts_with(prefix);
        }
        part == mime
    })
}

fn extension_of(filename: &str) -> String {
    filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

/// MIME type placed in `allow_upload` `type`.
pub fn mime_for_filename(filename: &str) -> &'static str {
    crate::config::mime_for_attachment_ext(&extension_of(filename))
}

/// File mtime in milliseconds, or 0.
pub fn last_modified_ms(path: &Path) -> u64 {
    let Ok(meta) = std::fs::metadata(path) else { return 0 };
    let Ok(modified) = meta.modified() else { return 0 };
    modified.duration_since(std::time::UNIX_EPOCH).map(|d| u64::try_from(d.as_millis()).unwrap_or(0)).unwrap_or(0)
}

/// Accept list seen on that input in the composer render. Dry-run warnings use it. The live form wins.
pub fn captured_accept(input_name: &str) -> &'static str {
    match input_name {
        "file_attachments" => ".pdf",
        _ => ".avif,.gif,.jpg,.jpeg,.png,.tiff,.webp,.mkv,.mov,.mp4,.webm",
    }
}

/// Refuse when the reply's limits are tighter than the files about to be sent.
pub fn limit_refusal(config: &AllowConfig, files: &[(&str, u64)]) -> Option<String> {
    if files.len() as u64 > config.max_entries {
        return Some(format!(
            "the site allows {} files on this input and {} were offered. Nothing was sent.",
            config.max_entries,
            files.len()
        ));
    }
    for (name, size) in files {
        if *size > config.max_file_size {
            return Some(format!(
                "{name} is {size} bytes and the site allows {}. Nothing was sent.",
                config.max_file_size
            ));
        }
    }
    None
}

/// Dry-run upload plan. No socket, no file bytes, and no token.
pub fn upload_plan_text(plan: &WritePlan) -> String {
    let mut s = String::new();
    let site: Vec<&PlannedAttachment> = plan.attachments.iter().filter(|f| f.on_site).collect();
    if site.is_empty() {
        return s;
    }
    s.push_str(&format!(
        "\nupload plan (no connection; chunk_size {CAPTURED_CHUNK_SIZE} is the captured default and the server can change it):\n"
    ));
    let mut groups: Vec<(&str, Vec<(u64, &PlannedAttachment)>)> = Vec::new();
    for (ref_n, file) in site.into_iter().enumerate() {
        let ref_n = u64::try_from(ref_n).unwrap_or(u64::MAX);
        let input = file.kind.input_name();
        if let Some(group) = groups.iter_mut().find(|g| g.0 == input) {
            group.1.push((ref_n, file));
        } else {
            groups.push((input, vec![(ref_n, file)]));
        }
    }
    for (input, files) in groups {
        s.push_str(&format!("{input} ref={DRY_RUN_UPLOAD_REF}\n"));
        let entries: Vec<UploadEntry> = files
            .iter()
            .map(|(n, file)| UploadEntry {
                last_modified: 0,
                name: file.name.clone(),
                relative_path: String::new(),
                size: file.bytes,
                mime: mime_for_filename(&file.name).to_string(),
                entry_ref: n.to_string(),
            })
            .collect();
        s.push_str(&serde_json::to_string(&allow_upload_payload(DRY_RUN_UPLOAD_REF, plan.cid, &entries)).unwrap_or_default());
        s.push('\n');
        for (n, file) in &files {
            let chunks = chunk_spans(file.bytes, CAPTURED_CHUNK_SIZE);
            s.push_str(&format!(
                "- {} entry {n} {} {} bytes, {} chunk(s) of {CAPTURED_CHUNK_SIZE}\n",
                file.name,
                mime_for_filename(&file.name),
                file.bytes,
                chunks.len()
            ));
            if file.bytes == 0 {
                s.push_str(&format!("  warning: {} is empty and is refused before upload.\n", file.name));
            }
            if !accept_allows(captured_accept(input), &file.name) {
                s.push_str(&format!(
                    "  warning: {} is not in the captured accept list for {input} ({}). A live upload stops before save if the form's list rejects it. The file is not renamed.\n",
                    file.name,
                    captured_accept(input)
                ));
            }
        }
    }
    s.push_str("last_modified is the file mtime in milliseconds when the upload is sent. This printout uses 0.\n");
    s
}

struct Assigned<'a> {
    file: &'a PlannedAttachment,
    entry_ref: String,
    upload_ref: String,
}

fn pick_input<'a>(file: &PlannedAttachment, inputs: &'a [UploadInput]) -> Result<&'a UploadInput, String> {
    let preferred = file.kind.input_name();
    let ok: Vec<&UploadInput> = inputs.iter().filter(|i| accept_allows(&i.accept, &file.name)).collect();
    if let Some(hit) = ok.iter().copied().find(|i| i.name == preferred) {
        return Ok(hit);
    }
    if let Some(hit) = ok.first() {
        return Ok(*hit);
    }
    if inputs.is_empty() {
        return Err(format!("{} needs a {preferred} upload on the form. Nothing was sent.", file.name));
    }
    let lists = inputs.iter().map(|i| format!("{} [{}]", i.name, i.accept)).collect::<Vec<_>>().join("; ");
    Err(format!("the site rejected {}: not in the accept list ({lists}). Nothing was sent.", file.name))
}

/// Upload every on-site file, then return. The caller sends validate and save.
///
/// A refusal leaves the `lvu` topics and does not save. Chunks of one file are
/// sequential. A failed chunk is not retried.
pub async fn send_uploads(
    lv: &mut crate::liveview::Lv,
    plan: &WritePlan,
    rendered_html: &str,
    fallback_html: &str,
    progress: &mut (impl FnMut(&str) + Send),
) -> Result<()> {
    let mut inputs = upload_inputs(rendered_html);
    if inputs.is_empty() {
        inputs = upload_inputs(fallback_html);
    }
    let mut assigned: Vec<Assigned> = Vec::new();
    let mut n = 0u64;
    for file in &plan.attachments {
        if !file.on_site {
            continue;
        }
        if file.bytes == 0 {
            bail!("{} is empty. Nothing was sent.", file.name);
        }
        let input = pick_input(file, &inputs).map_err(|e| anyhow!("{e}"))?;
        assigned.push(Assigned {
            file,
            entry_ref: n.to_string(),
            upload_ref: input.upload_ref.clone(),
        });
        n += 1;
    }
    if assigned.is_empty() {
        return Ok(());
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, row) in assigned.iter().enumerate() {
        if let Some(group) = groups.iter_mut().find(|g| assigned[g[0]].upload_ref == row.upload_ref) {
            group.push(i);
        } else {
            groups.push(vec![i]);
        }
    }
    let mut joined: Vec<(String, String)> = Vec::new();
    for group in &groups {
        if let Err(e) = upload_group(lv, plan.cid, &assigned, group, &mut joined, progress).await {
            for (join_ref, topic) in joined.iter().rev() {
                lv.leave_topic(join_ref, topic).await;
            }
            return Err(e);
        }
    }
    Ok(())
}

async fn upload_group(
    lv: &mut crate::liveview::Lv,
    cid: u64,
    assigned: &[Assigned<'_>],
    group: &[usize],
    joined: &mut Vec<(String, String)>,
    progress: &mut (impl FnMut(&str) + Send),
) -> Result<()> {
    let upload_ref = &assigned[group[0]].upload_ref;
    let entries: Vec<UploadEntry> = group
        .iter()
        .map(|&i| {
            let row = &assigned[i];
            UploadEntry {
                last_modified: last_modified_ms(&row.file.path),
                name: row.file.name.clone(),
                relative_path: String::new(),
                size: row.file.bytes,
                mime: mime_for_filename(&row.file.name).to_string(),
                entry_ref: row.entry_ref.clone(),
            }
        })
        .collect();
    let payload = allow_upload_payload(upload_ref, cid, &entries);
    let reply = lv.push("allow_upload", payload).await.context("allow_upload")?;
    let refs: Vec<&str> = entries.iter().map(|e| e.entry_ref.as_str()).collect();
    let (config, tokens) = parse_allow_reply(&reply, &refs).map_err(|e| anyhow!("{e}"))?;
    let sizes: Vec<(&str, u64)> = entries.iter().map(|e| (e.name.as_str(), e.size)).collect();
    if let Some(reason) = limit_refusal(&config, &sizes) {
        bail!("{reason}");
    }
    let timeout = Duration::from_millis(config.chunk_timeout_ms);
    for (entry, &i) in entries.iter().zip(group.iter()) {
        let row = &assigned[i];
        let Some(token) = tokens.get(&entry.entry_ref) else {
            bail!("the site did not return an upload token for {}. Nothing was sent.", entry.name);
        };
        let topic = format!("lvu:{}", entry.entry_ref);
        let (join_ref, join_reply) = lv
            .join_topic(&topic, json!({"token": token}))
            .await
            .context("joining the upload channel")?;
        if join_reply.get("status").and_then(Value::as_str) != Some("ok") {
            bail!("the site rejected the upload channel for {} ({}). Nothing was sent.", entry.name, brief(&join_reply.to_string()));
        }
        joined.push((join_ref.clone(), topic.clone()));
        send_chunks(
            lv,
            &ChunkJob {
                file: row.file,
                entry_ref: &entry.entry_ref,
                upload_ref,
                join_ref: &join_ref,
                topic: &topic,
                chunk_size: config.chunk_size,
                timeout,
                cid,
            },
            progress,
        )
        .await?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ChunkJob<'a> {
    file: &'a PlannedAttachment,
    entry_ref: &'a str,
    upload_ref: &'a str,
    join_ref: &'a str,
    topic: &'a str,
    chunk_size: u64,
    timeout: Duration,
    cid: u64,
}

async fn send_chunks(lv: &mut crate::liveview::Lv, job: &ChunkJob<'_>, progress: &mut (impl FnMut(&str) + Send)) -> Result<()> {
    let ChunkJob { file, entry_ref, upload_ref, join_ref, topic, chunk_size, timeout, cid } = *job;
    let mut reader = std::fs::File::open(&file.path).with_context(|| format!("reading {}", file.name))?;
    let mut left = file.bytes;
    let mut sent = 0u64;
    let mut last_shown: Option<u64> = None;
    let mut sent_any = false;
    while left > 0 {
        let n = usize::try_from(chunk_size.min(left)).unwrap_or(usize::MAX);
        let mut buf = vec![0u8; n];
        let mut filled = 0usize;
        while filled < n {
            match std::io::Read::read(&mut reader, &mut buf[filled..]) {
                Ok(0) => break,
                Ok(k) => filled += k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => bail!("could not read {}: {e}. {}", file.name, stop_line(sent_any)),
            }
        }
        if filled == 0 {
            bail!("{} ended early ({} of {} bytes). {}", file.name, sent, file.bytes, stop_line(sent_any));
        }
        buf.truncate(filled);
        let reply = match lv.push_binary(join_ref, topic, "chunk", &buf, timeout).await {
            Ok(reply) => reply,
            Err(e) => {
                let msg = brief(&format!("{e:#}"));
                bail!("upload of {} failed ({msg}). {}", file.name, stop_line(sent_any));
            }
        };
        if reply.get("status").and_then(Value::as_str) != Some("ok") {
            bail!(
                "the site rejected a chunk of {} ({}). {}",
                file.name,
                brief(&reply.to_string()),
                stop_line(true)
            );
        }
        sent += filled as u64;
        left -= filled as u64;
        sent_any = true;
        let pct = upload_percent(sent, file.bytes);
        let prog = lv.push("progress", progress_payload(upload_ref, entry_ref, pct, cid)).await.context("progress")?;
        if prog.get("status").and_then(Value::as_str) != Some("ok") {
            bail!(
                "the site rejected progress for {} ({}). {}",
                file.name,
                brief(&prog.to_string()),
                stop_line(true)
            );
        }
        if last_shown != Some(pct) {
            progress(&format!("uploading {} {pct}%", file.name));
            last_shown = Some(pct);
        }
    }
    if sent != file.bytes {
        bail!("{} sent {sent} of {} bytes. {}", file.name, file.bytes, stop_line(sent_any));
    }
    Ok(())
}

fn stop_line(sent_any: bool) -> &'static str {
    if sent_any {
        "Nothing was retried. The post was not saved."
    } else {
        "Nothing was sent."
    }
}

/// Which input a file would use. Tests the accept routing without a socket.
#[cfg(test)]
pub fn route_name(kind: AttachmentKind, filename: &str, inputs: &[UploadInput]) -> Result<String, String> {
    let file = PlannedAttachment {
        path: Path::new(filename).to_path_buf(),
        name: filename.to_string(),
        kind,
        bytes: 1,
        on_site: true,
    };
    pick_input(&file, inputs).map(|i| i.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/site/upload.json")).unwrap()
    }

    fn entry(name: &str, size: u64, mime: &str, entry_ref: &str) -> UploadEntry {
        UploadEntry {
            last_modified: 1_700_000_000_000,
            name: name.into(),
            relative_path: String::new(),
            size,
            mime: mime.into(),
            entry_ref: entry_ref.into(),
        }
    }

    #[test]
    fn the_fixture_matches_the_allow_upload_shape_and_carries_no_live_secret() {
        let raw = include_str!("../../tests/fixtures/site/upload.json");
        assert!(!raw.contains("SFMyNTY"), "{raw}");
        assert!(!raw.contains("eyJ"), "{raw}");
        assert!(!raw.contains("phx-GOCE"), "{raw}");
        assert!(!raw.contains("AaFeCe"), "{raw}");
        let doc = fixture();
        let entries = vec![
            entry("photo.jpg", 1200, "image/jpeg", "0"),
            entry("clip.mp4", 128_000, "video/mp4", "1"),
        ];
        let payload = allow_upload_payload("phx-UPLOADREF", 1, &entries);
        assert_eq!(payload, doc["allow_upload"]["payload"]);
        assert!(payload.get("uploads").is_none());
        assert_eq!(payload["entries"][0]["relative_path"], "");
        assert_eq!(payload["entries"][0]["type"], "image/jpeg");
        assert_eq!(doc["allow_reply"]["response"]["config"]["chunk_size"], CAPTURED_CHUNK_SIZE);
        assert_eq!(doc["allow_reply"]["response"]["config"]["max_entries"], 4);
        assert_eq!(doc["allow_reply"]["response"]["config"]["max_file_size"], CAPTURED_MAX_FILE_SIZE);
        assert_eq!(doc["allow_reply"]["response"]["config"]["chunk_timeout"], CAPTURED_CHUNK_TIMEOUT_MS);
        assert!(doc["allow_reply"]["response"]["errors"].as_object().unwrap().is_empty());
        let (cfg, tokens) = parse_allow_reply(&doc["allow_reply"], &["0", "1"]).unwrap();
        assert_eq!(cfg.chunk_size, CAPTURED_CHUNK_SIZE);
        assert_eq!(tokens.get("0").map(String::as_str), Some("synthetic-token-entry-0"));
        assert_eq!(tokens.get("1").map(String::as_str), Some("synthetic-token-entry-1"));
        let join = lvu_join_message("10", "lvu:0", "synthetic-token-entry-0");
        assert_eq!(join, doc["lvu_join"]);
        assert_eq!(join[0], join[1], "the upload join ref is the message ref");
        assert_eq!(progress_payload("phx-UPLOADREF", "0", 100, 1), doc["progress"]);
        assert!(doc["progress"]["event"].is_null());
        assert_eq!(doc["save_has_uploads_key"], false);
    }

    #[test]
    fn a_pdf_uses_the_file_input_and_the_same_preflight() {
        let doc = fixture();
        let pdf = entry("notes.pdf", 800, "application/pdf", "0");
        let payload = allow_upload_payload("phx-PDFREF", 1, &[pdf]);
        assert_eq!(payload, doc["pdf"]["allow_upload"]);
        assert_eq!(payload["entries"][0]["type"], "application/pdf");
        assert_eq!(doc["pdf"]["input"], "file_attachments");
        assert_eq!(doc["pdf"]["accept"], ".pdf");
        assert!(accept_allows(".pdf", "notes.pdf"));
        assert!(!accept_allows(captured_accept("media_attachments"), "notes.pdf"));
        let inputs = upload_inputs(
            r#"<input type="file" name="media_attachments" accept=".avif,.gif,.jpg,.jpeg,.png,.tiff,.webp,.mkv,.mov,.mp4,.webm" data-phx-upload-ref="phx-UPLOADREF">
               <div data-phx-upload-ref="phx-UPLOADREF"></div>
               <input type="file" name="file_attachments" accept=".pdf" data-phx-upload-ref="phx-PDFREF">"#,
        );
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].name, "media_attachments");
        assert_eq!(inputs[1].name, "file_attachments");
        assert_eq!(route_name(AttachmentKind::File, "notes.pdf", &inputs).unwrap(), "file_attachments");
        assert_eq!(route_name(AttachmentKind::Media, "photo.jpg", &inputs).unwrap(), "media_attachments");
        let err = route_name(AttachmentKind::Media, "scan.tif", &inputs).unwrap_err();
        assert!(err.contains("scan.tif"), "{err}");
        assert!(err.contains("Nothing was sent"), "{err}");
        assert!(!err.contains("renamed"), "{err}");
        assert!(accept_allows(captured_accept("media_attachments"), "scan.tiff"));
        assert!(accept_allows(captured_accept("media_attachments"), "photo.jpeg"));
    }

    #[test]
    fn errors_and_a_missing_token_stop_before_any_chunk() {
        let doc = fixture();
        let err = parse_allow_reply(&doc["errors_example"], &["0"]).unwrap_err();
        assert!(err.contains("too_large"), "{err}");
        assert!(err.contains("Nothing was sent"), "{err}");
        assert!(!err.contains("synthetic-token"), "{err}");
        assert!(!err.contains("\"event\": \"save\""), "{err}");
        let mut reply = doc["allow_reply"].clone();
        reply["response"]["entries"] = json!({});
        reply["response"]["errors"] = json!({"0": ["too_large"]});
        let err = parse_allow_reply(&reply, &["0"]).unwrap_err();
        assert!(err.contains("too_large"), "{err}");
        assert!(!err.contains("synthetic-token-entry-0"), "{err}");
        reply["response"]["errors"] = json!({});
        let missing = parse_allow_reply(&reply, &["0"]).unwrap_err();
        assert!(missing.contains("token"), "{missing}");
        assert!(missing.contains("Nothing was sent"), "{missing}");
        let tight = AllowConfig { chunk_size: 64_000, chunk_timeout_ms: 10_000, max_entries: 1, max_file_size: 10 };
        let reason = limit_refusal(&tight, &[("photo.jpg", 11)]).unwrap();
        assert!(reason.contains("photo.jpg"), "{reason}");
        assert!(reason.contains("Nothing was sent"), "{reason}");
    }

    #[test]
    fn chunk_headers_round_trip_and_percent_matches_the_capture() {
        let payload = vec![7u8; 64_000];
        let frame = crate::liveview::encode_binary_push("106", "110", "lvu:1", "chunk", &payload).unwrap();
        assert_eq!(frame.len(), 21 + 64_000);
        assert_eq!(frame[0], 0);
        let (kind, join_ref, msg_ref, topic, event, body) = crate::liveview::decode_binary_push(&frame).unwrap();
        assert_eq!(kind, 0);
        assert_eq!(join_ref, "106");
        assert_eq!(msg_ref, "110");
        assert_eq!(topic, "lvu:1");
        assert_eq!(event, "chunk");
        assert_eq!(body.len(), 64_000);
        assert_eq!(upload_percent(64_000, 1_019_521), 6);
        assert_eq!(upload_percent(64_000, 96_637_871), 0);
        assert_eq!(upload_percent(1_019_521, 1_019_521), 100);
        assert_eq!(chunk_spans(64_001, 64_000), vec![(0, 64_000), (64_000, 1)]);
        assert!(chunk_spans(0, 64_000).is_empty());
    }
}
