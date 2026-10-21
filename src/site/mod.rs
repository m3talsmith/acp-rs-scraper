//! Write core (3.4.0), attachments (3.5.0), text publish (3.31.0), uploads (3.32.0),
//! and reply edit (3.33.0).
//!
//! The safety layer, the Markdown to ProseMirror writer, and the reply frames
//! live here. A confirmed post or reply uploads the site copy, then sends
//! validate, then save, once. Images, videos, and PDFs are the site copy, at
//! most 4. A confirmed reply edit sends validate, then save, once. Vote,
//! subscribe, delete, repost, and a top-level post edit stay behind
//! [`write::CAPTURE_BLOCK`]. The site does not support changing attachments
//! on an edit. The one-hour window still refuses an
//! older edit before a connection. `--dry-run` opens no connection.

mod attach;
pub mod delete;
pub mod edit;
pub mod repost;
pub mod subscribe;
pub mod vote;
mod prosemirror;
mod upload;
mod write;

pub use attach::{
    AttachmentKind, PathCompletion, PlannedAttachment, SiteSplit, complete_path, expand_with_home, plan_attachment, plan_attachments,
    split_for_site,
};
pub use prosemirror::{BODY_CHAR_LIMIT, assemble, markdown_to_doc, plain_blocks, post_ref};
pub use write::{
    CAPTURE_BLOCK, Completion, Draft, Gate, Kind, NickHint, Probe, Published, WriteError, WritePlan, bundle_changed, capture_stub_message,
    after_reply, completions, decide, dry_run_text, event_payloads, interpret, liveview_frames, load_last_write, prepare, probe_html, publish_text, published_text, rate_wait, refuse_live_write, store_last_write, writes_enabled,
    writes_enabled_from,
};

use crate::prelude::*;

/// Headless New post / Reply / `--dry-run` / `--capture-reply`.
pub fn cli_requested(args: &Args) -> bool {
    // `--dry-run` and `--yes` also belong to --remove-nick / --clear-all.
    args.capture_reply.is_some()
        || args.new_post.is_some()
        || args.reply.is_some()
        || !args.attachment.is_empty()
        || args.body.is_some()
        || args.body_file.is_some()
        || args.in_reply_to.is_some()
        || args.quote.is_some()
        || args.quote_post.is_some()
        || args.accept_site_update
        || args.site_attachments.is_some()
        || args.repost.is_some()
        || args.delete.is_some()
        || args.edit.is_some()
        || args.subscribe.is_some()
        || args.vote.is_some()
}

/// `y/N`, then `yes`. `--yes` covers both. A dry-run does not ask.
fn confirm_strict(args: &Args, prompt: &str) -> Result<bool> {
    if !confirm_write(args, prompt)? {
        return Ok(false);
    }
    if args.yes {
        return Ok(true);
    }
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        eprint!("Type yes: ");
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        return Ok(line.trim() == "yes");
    }
    Ok(false)
}

fn confirm_write(args: &Args, prompt: &str) -> Result<bool> {
    if args.dry_run {
        return Ok(false);
    }
    if args.yes {
        return Ok(true);
    }
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        eprint!("{prompt}");
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        return Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"));
    }
    Ok(false)
}

/// Run a write command. Cancel prints `nothing sent` and returns Ok.
/// A confirmed text post or reply logs in and sends validate, then save, once.
pub async fn run_cli(args: &Args) -> Result<()> {
    if let Some(thread) = args.capture_reply.as_deref() {
        bail!("{}", capture_stub_message(thread.trim()));
    }
    if let Some(id) = args.vote.as_deref() {
        let state = args.vote_state.as_deref().unwrap_or("1");
        let confirmed = confirm_write(args, "Vote? [y/N] ")?;
        return match vote::present(id.trim(), state, confirmed, args.dry_run) {
            Ok(text) => {
                println!("{text}");
                Ok(())
            }
            Err(e) => bail!("{e}"),
        };
    }
    if let Some(id) = args.subscribe.as_deref() {
        let state = args.subscribe_state.as_deref().unwrap_or("1");
        let confirmed = confirm_write(args, "Subscribe? [y/N] ")?;
        return match subscribe::present(id.trim(), state, confirmed, args.dry_run) {
            Ok(text) => {
                println!("{text}");
                Ok(())
            }
            Err(e) => bail!("{e}"),
        };
    }
    if let Some(id) = args.delete.as_deref() {
        let confirmed = confirm_strict(args, "Delete? [y/N] ")?;
        return match delete::present(id.trim(), confirmed, args.dry_run) {
            Ok(text) => {
                println!("{text}");
                Ok(())
            }
            Err(e) => bail!("{e}"),
        };
    }
    if let Some(id) = args.edit.as_deref() {
        if args.quote.is_some() {
            bail!("a reply edit takes the body only. A quote is not added. Nothing was sent.");
        }
        if !args.attachment.is_empty() {
            bail!("{}", edit::ATTACHMENT_EDIT_REFUSAL);
        }
        let body = match (&args.body, &args.body_file) {
            (Some(_), Some(_)) => bail!("pass --body or --body-file, not both"),
            (Some(s), None) => s.clone(),
            (None, Some(path)) => std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
            (None, None) => String::new(),
        };
        let confirmed = confirm_strict(args, "Edit? [y/N] ")?;
        let now = chrono::Utc::now().timestamp();
        return match edit::gate(id.trim(), &body, args.posted_at, now, confirmed, args.dry_run, writes_enabled()) {
            Ok(edit::EditGate::Done(text)) => {
                println!("{text}");
                Ok(())
            }
            Ok(edit::EditGate::Send(plan)) => {
                let jar = Arc::new(Jar::default());
                if let Err(e) = load_session(&jar) {
                    cli_warning_session(&e);
                }
                let http = client(jar.clone())?;
                login(&http, &jar).await?;
                let published = edit::publish_reply_edit(&http, &jar, &plan).await?;
                println!("published {}\n{}", published.id, published.url);
                Ok(())
            }
            Err(e) => bail!("{e}"),
        };
    }
    if let Some(id) = args.repost.as_deref() {
        let confirmed = if args.dry_run {
            false
        } else if args.yes {
            true
        } else if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            eprint!("Repost? [y/N] ");
            io::stderr().flush()?;
            let mut line = String::new();
            io::stdin().read_line(&mut line)?;
            matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
        } else {
            false
        };
        return match repost::present(id.trim(), confirmed, args.dry_run) {
            Ok(text) => {
                println!("{text}");
                Ok(())
            }
            Err(e) => bail!("{e}"),
        };
    }
    if args.new_post.is_none() && args.reply.is_none() {
        bail!("a write needs --new-post TITLE or --reply THREAD. --dry-run prints the frames and does not connect.");
    }
    let body = match (&args.body, &args.body_file) {
        (Some(_), Some(_)) => bail!("pass --body or --body-file, not both"),
        (Some(s), None) => s.clone(),
        (None, Some(path)) => std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
        (None, None) => String::new(),
    };
    let draft = Draft {
        title: args.new_post.clone().unwrap_or_default(),
        body,
        thread: args.reply.clone(),
        in_reply_to: args.in_reply_to.clone(),
        quote: args.quote.clone(),
        quote_post: args.quote_post.clone(),
        attachments: args.attachment.clone(),
        site_attachments: args.site_attachments.as_ref().map(|list| list.0.clone()),
        html: None,
        accept_bundle: args.accept_site_update,
        stored_bundle: None,
        now_unix: chrono::Utc::now().timestamp(),
        last_write_unix: None,
        writes_on: writes_enabled(),
    };
    let confirmed = if args.dry_run {
        false
    } else if args.yes {
        true
    } else if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        eprint!("Publish? [y/N] ");
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
    } else {
        false
    };
    let mut draft = draft;
    if confirmed && !args.dry_run {
        draft.last_write_unix = draft.last_write_unix.or_else(load_last_write);
    }
    match decide(&draft, confirmed, args.dry_run) {
        Ok(Gate::Done(text)) => {
            println!("{text}");
            Ok(())
        }
        Ok(Gate::Send(plan)) => {
            store_last_write(draft.now_unix);
            let jar = Arc::new(Jar::default());
            if let Err(e) = load_session(&jar) {
                cli_warning_session(&e);
            }
            let http = client(jar.clone())?;
            login(&http, &jar).await?;
            let published = publish_text(&http, &jar, &plan, draft.accept_bundle, |line| eprintln!("{line}")).await?;
            println!("{}", published_text(&published, &plan));
            Ok(())
        }
        Err(e) => bail!("{e}"),
    }
}

fn cli_warning_session(err: &anyhow::Error) {
    eprintln!("ignoring saved session: {err:#}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactions_stay_gated_until_the_click_is_captured() {
        let notes = include_str!("../../architecture/INTERACTIONS-CAPTURE.md");
        assert!(notes.contains("stay behind `CAPTURE_BLOCK`"), "{notes}");
        assert!(notes.contains("https://acp.us/rs/post/POST"));
        assert!(notes.contains("upvote control"));
        assert!(notes.contains("https://acp.us/rs/post/THREAD"));
        assert!(notes.contains("subscribe control"));
        assert!(notes.contains("click Delete"));
        assert!(notes.contains("https://acp.us/rs/post/POST/edit"));
        assert!(notes.contains("Change only the title"));
        assert!(notes.contains("does not support changing attachments"));
        assert!(!notes.contains("Edit: replace attachment"), "{notes}");
        assert!(notes.contains("/rs/post/new?repost=POST"));
        assert!(notes.contains("short quote"));
        let edit: Value = serde_json::from_str(include_str!("../../tests/fixtures/site/edit.json")).unwrap();
        assert_eq!(edit["capture_status"], "reply_body");
        assert!(edit["still_needed"].as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains("title")));
        for (name, raw) in [
            ("vote", include_str!("../../tests/fixtures/site/vote.json")),
            ("subscribe", include_str!("../../tests/fixtures/site/subscribe.json")),
            ("delete", include_str!("../../tests/fixtures/site/delete.json")),
            ("repost", include_str!("../../tests/fixtures/site/repost.json")),
        ] {
            let doc: Value = serde_json::from_str(raw).unwrap();
            assert_eq!(doc["capture_status"], "missing", "{name}");
            assert!(CAPTURE_BLOCK.contains("are not sent"));
        }
    }

    fn draft() -> Draft {
        Draft {
            title: "Strike".into(),
            body: "hello **world**".into(),
            thread: None,
            in_reply_to: None,
            quote: None,
            quote_post: None,
            attachments: Vec::new(),
            site_attachments: None,
            html: None,
            accept_bundle: false,
            stored_bundle: None,
            now_unix: 1_000,
            last_write_unix: None,
            writes_on: true,
        }
    }

    fn shown(g: Result<Gate, WriteError>) -> String {
        match g.unwrap() {
            Gate::Done(s) => s,
            Gate::Send(_) => panic!("dry-run or cancel tried to send"),
        }
    }

    #[test]
    fn markdown_round_trips_through_the_existing_reader() {
        let doc = markdown_to_doc("hello **world** and a [link](https://example.test/a).").unwrap();
        let mut plain = String::new();
        crate::scrape::prosemirror_text(&doc, &mut plain);
        let plain = crate::scrape::tidy_text(&plain);
        assert!(plain.contains("hello world and a link"), "{plain}");
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        assert!(nodes.iter().any(|n| n["marks"].as_array().is_some_and(|m| m.iter().any(|x| x["type"] == "bold"))));
        assert!(nodes.iter().any(|n| {
            n["marks"].as_array().is_some_and(|m| m.iter().any(|x| x["attrs"]["href"] == "https://example.test/a"))
        }));
    }

    #[test]
    fn mentions_and_tags_are_nodes_and_post_anchors_stay_text() {
        let doc = markdown_to_doc("see @ada and #strike but not #post-abc").unwrap();
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        assert!(nodes.iter().any(|n| n["type"] == "mention" && n["attrs"]["label"] == "@ada" && n["attrs"]["id"] == ""));
        assert!(nodes.iter().any(|n| n["type"] == "mention" && n["attrs"]["label"] == "#strike" && n["attrs"]["mentionSuggestionChar"] == "#"));
        let text = nodes.iter().filter(|n| n["type"] == "text").map(|n| n["text"].as_str().unwrap()).collect::<String>();
        assert!(text.contains("#post-abc"), "{text}");
    }

    #[test]
    fn unsupported_markdown_is_rejected() {
        for (md, needle) in [
            ("# Title", "heading"),
            ("`code`", "code"),
            ("```\ncode\n```", "code block"),
            ("~~no~~", "strikethrough"),
            ("| a |\n| - |\n| b |", "table"),
            ("![x](https://example.test/a.png)", "image"),
        ] {
            let msg = markdown_to_doc(md).unwrap_err().message;
            assert!(msg.contains(needle), "{md} → {msg}");
        }
    }

    #[test]
    fn lists_tasks_quotes_and_line_breaks() {
        let doc = markdown_to_doc("- one\n- two").unwrap();
        assert_eq!(doc["content"][0]["type"], "bulletList");
        let doc = markdown_to_doc("1. first").unwrap();
        assert_eq!(doc["content"][0]["type"], "orderedList");
        let doc = markdown_to_doc("- [x] done\n- [ ] not").unwrap();
        assert_eq!(doc["content"][0]["type"], "taskList");
        assert_eq!(doc["content"][0]["content"][0]["attrs"]["checked"], true);
        let doc = markdown_to_doc("> quoted\n\nline one\nline two").unwrap();
        assert_eq!(doc["content"][0]["type"], "blockquote");
        assert!(doc["content"][1]["content"].as_array().unwrap().iter().any(|n| n["type"] == "hardBreak"));
        let doc = markdown_to_doc("see <u>under</u>").unwrap();
        assert!(doc.to_string().contains("underline"), "{doc}");
    }

    #[test]
    fn reply_to_a_comment_prefixes_post_ref_and_a_quote() {
        let mut d = draft();
        d.title.clear();
        d.thread = Some("T1".into());
        d.in_reply_to = Some("C1".into());
        d.quote = Some("selected lines".into());
        d.body = "agreed".into();
        let plan = prepare(&d).unwrap();
        assert_eq!(plan.kind, Kind::Reply);
        assert_eq!(plan.page, "/rs/post/T1");
        assert_eq!(plan.cid, 2);
        assert_eq!(plan.body["content"][0]["type"], "postRef");
        assert_eq!(plan.body["content"][0]["attrs"]["postId"], "C1");
        assert_eq!(plan.body["content"][0]["attrs"]["threadId"], "T1");
        assert_eq!(plan.body["content"][1]["type"], "blockquote");
        let names: Vec<_> = plan.fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["form[thread_id]", "form[body_input]"]);
        let body = &plan.fields[1].1;
        assert!(body.starts_with(r#"{"type":"doc","content":[{"type":"postRef""#), "{body}");
        assert!(body.contains("agreed"));
        let frames = liveview_frames(&plan);
        assert_eq!(frames.len(), 2, "a validate after save is not built");
        assert_eq!(frames[0][4]["event"], "validate");
        assert_eq!(frames[0][4]["meta"]["_target"], "form[body_input]");
        assert!(frames[0][4]["uploads"].as_object().unwrap().is_empty());
        assert_eq!(frames[1][4]["event"], "save");
        assert!(frames[1][4]["meta"].as_object().unwrap().is_empty());
        assert!(frames[1][4].get("uploads").is_none());
        assert!(!frames[1][4]["value"].as_str().unwrap().contains("_csrf_token"));
    }

    #[test]
    fn reply_to_the_thread_root_starts_with_post_ref() {
        let mut d = draft();
        d.title.clear();
        d.thread = Some("T1".into());
        d.body = "hello".into();
        let plan = prepare(&d).unwrap();
        assert_eq!(plan.body["content"][0]["type"], "postRef");
        assert_eq!(plan.body["content"][0]["attrs"]["postId"], "T1");
        assert_eq!(plan.body["content"][0]["attrs"]["threadId"], "T1");
        assert_eq!(plan.body["content"][1]["type"], "paragraph");
        let names: Vec<_> = plan.fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["form[thread_id]", "form[body_input]"]);
        assert_eq!(plan.fields[0].1, "T1");
    }

    #[test]
    fn dry_run_prints_frames_and_cancel_sends_nothing() {
        let d = draft();
        let text = shown(decide(&d, false, true));
        assert!(text.contains("dry-run: no connection opened"), "{text}");
        assert!(text.contains("\"event\": \"validate\""), "{text}");
        assert!(text.contains("\"event\": \"save\""), "{text}");
        assert!(text.contains("form%5Btitle%5D=Strike") || text.contains("form[title]"), "{text}");
        assert!(text.contains("_target"), "{text}");
        assert!(!text.contains("https://acp.us"), "{text}");
        assert_eq!(shown(decide(&d, false, false)), "nothing sent");
    }

    #[test]
    fn confirm_prepares_a_text_send_and_does_not_open_a_connection() {
        let Gate::Send(plan) = decide(&draft(), true, false).unwrap() else {
            panic!("a text post is ready to send");
        };
        let payloads = event_payloads(&plan);
        assert_eq!(payloads.len(), 2);
        assert_eq!(payloads[0]["event"], "validate");
        assert_eq!(payloads[1]["event"], "save");
        assert!(payloads[0].get("meta").is_none(), "a new post does not invent _target");
        assert!(!payloads[0].to_string().contains("_target"));
        assert!(payloads[0]["value"].as_str().unwrap().contains("form%5Btitle%5D="));
        assert!(!plan.page.starts_with("http"));
        assert!(plan.attachments.is_empty());
    }

    #[test]
    fn writes_off_attachments_bundle_and_rate_limit_block() {
        let mut d = draft();
        d.writes_on = false;
        assert!(prepare(&d).unwrap_err().message.contains("writes are off"));
        d.writes_on = true;
        d.html = Some(FORM.into());
        d.stored_bundle = Some("/assets/app-old.js".into());
        let err = decide(&d, true, false).unwrap_err();
        assert!(err.message.contains("bundle changed"), "{}", err.message);
        d.accept_bundle = true;
        let Gate::Send(_) = decide(&d, true, false).unwrap() else {
            panic!("accepted bundle, text only, is ready to send");
        };
        d.accept_bundle = false;
        d.stored_bundle = None;
        d.last_write_unix = Some(980);
        d.now_unix = 1_000;
        assert!(decide(&d, true, false).unwrap_err().message.contains("rate limit"));
        assert!(decide(&d, false, true).is_ok(), "dry-run does not spend the bucket");
    }

    #[test]
    fn probe_reads_the_form_and_does_not_invent_hidden_fields() {
        let p = probe_html(FORM);
        assert!(p.found);
        assert_eq!(p.submit.as_deref(), Some("save"));
        assert_eq!(p.change.as_deref(), Some("validate"));
        assert_eq!(p.cid, Some(2));
        assert_eq!(p.script_src.as_deref(), Some("/assets/app-abc123.js"));
        assert_eq!(p.hidden, vec![("_csrf_token".into(), "SECRETSECRETSECRETSECRETSECRET12".into())]);
        let mut d = draft();
        d.title.clear();
        d.thread = Some("T1".into());
        d.body = "hi".into();
        d.html = Some(FORM.into());
        let plan = prepare(&d).unwrap();
        assert_eq!(plan.cid, 2);
        let names: Vec<_> = plan.fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["form[thread_id]", "form[body_input]"]);
        let shown = dry_run_text(&plan);
        assert!(!shown.contains("SECRETSECRETSECRETSECRETSECRET12"), "{shown}");
        assert!(!shown.contains("_csrf_token"), "{shown}");
    }

    #[test]
    fn empty_and_overlong_bodies_are_refused() {
        let mut d = draft();
        d.body.clear();
        assert!(prepare(&d).unwrap_err().message.contains("empty"));
        d.body = "x".repeat(BODY_CHAR_LIMIT + 1);
        assert!(prepare(&d).unwrap_err().message.contains("40000"));
    }

    #[test]
    fn completion_is_local() {
        let nicks = vec![NickHint { nick: "ada".into(), user_id: String::new() }, NickHint { nick: "bob".into(), user_id: "9".into() }];
        let tags = vec!["strike".into(), "post-skip".into()];
        let rows = completions("@ad", &nicks, &tags);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "@ada");
        assert_eq!(rows[0].id, "");
        assert!(completions("#post", &nicks, &tags).is_empty());
        assert_eq!(completions("#str", &nicks, &tags)[0].label, "#strike");
    }

    #[test]
    fn the_write_switch_lets_the_environment_win() {
        assert!(writes_enabled_from(None, None));
        assert!(!writes_enabled_from(None, Some("off")));
        assert!(!writes_enabled_from(Some(true), Some("0")));
        assert!(writes_enabled_from(Some(false), Some("on")));
        assert!(!writes_enabled_from(Some(false), None));
    }

    #[test]
    #[should_panic(expected = "write path contacted")]
    fn tests_panic_if_a_write_targets_the_real_site() {
        let _ = refuse_live_write("https://acp.us/rs/post/T1");
    }

    #[test]
    fn a_non_site_host_is_still_refused_without_panicking() {
        let err = refuse_live_write("http://127.0.0.1/rs/post/new").unwrap_err();
        assert!(err.to_string().contains("are not sent"));
        assert!(err.to_string().contains("text posts and replies are published"));
    }

    #[test]
    fn site_state_stores_the_bundle_key() {
        let c = rusqlite::Connection::open_in_memory().unwrap();
        assert!(crate::storage::db::site_state_get(&c, "bundle").unwrap().is_none());
        crate::storage::db::site_state_set(&c, "bundle", "/assets/app.js").unwrap();
        assert_eq!(crate::storage::db::site_state_get(&c, "bundle").unwrap().as_deref(), Some("/assets/app.js"));
    }

    #[test]
    fn maintenance_dry_run_is_not_a_write_command() {
        use clap::Parser;
        let a = Args::try_parse_from(["x", "--remove-nick", "ada", "--dry-run", "-y"]).unwrap();
        assert!(!cli_requested(&a));
        let a = Args::try_parse_from(["x", "--clear-all", "--yes"]).unwrap();
        assert!(!cli_requested(&a));
        let a = Args::try_parse_from(["x", "--new-post", "Title", "--dry-run"]).unwrap();
        assert!(cli_requested(&a));
        assert!(Args::try_parse_from(["x", "--new-post", "Title", "--remove-nick", "ada"]).is_err());
    }

    #[test]
    fn attachments_are_listed_in_dry_run_and_not_encoded() {
        let dir = std::env::temp_dir().join(format!("acp-attach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let jpg = dir.join("photo.jpg");
        let pdf = dir.join("notes.pdf");
        let bad = dir.join("notes.txt");
        std::fs::write(&jpg, b"jpeg-bytes").unwrap();
        std::fs::write(&pdf, b"pdf").unwrap();
        std::fs::write(&bad, b"nope").unwrap();
        let mut d = draft();
        d.attachments = vec![jpg.clone(), pdf.clone(), bad.clone()];
        let plan = prepare(&d).unwrap();
        assert_eq!(plan.attachments.len(), 3);
        assert!(plan.attachments[0].on_site);
        assert!(plan.attachments[1].on_site, "a pdf is a site file");
        assert!(!plan.attachments[2].on_site, "a text file stays on the local copy");
        assert!(plan.site_notice.is_none());
        assert_eq!(plan.attachments[0].kind, AttachmentKind::Media);
        assert_eq!(plan.attachments[0].kind.input_name(), "media_attachments");
        assert_eq!(plan.attachments[1].kind, AttachmentKind::File);
        assert_eq!(plan.attachments[1].kind.input_name(), "file_attachments");
        assert_eq!(plan.attachments[1].bytes, 3);
        let frames = serde_json::to_string(&liveview_frames(&plan)).unwrap();
        assert!(!frames.contains("photo.jpg"), "{frames}");
        assert!(!frames.contains("allow_upload"), "{frames}");
        assert!(!frames.contains("jpeg-bytes"), "{frames}");
        assert!(plan.fields.iter().all(|(k, _)| !k.contains("attachment")), "{:?}", plan.fields);
        let text = shown(decide(&d, false, true));
        assert!(text.contains("the site copy is a subset"), "{text}");
        assert!(text.contains("site media photo.jpg"), "{text}");
        assert!(text.contains("site file notes.pdf"), "{text}");
        assert!(text.contains("local-only file notes.txt"), "{text}");
        assert!(text.contains("not uploaded (kept on this machine)"), "{text}");
        assert!(text.contains("allow_upload"), "{text}");
        assert!(text.contains("phx:<page>"), "{text}");
        assert!(text.contains("file_attachments"), "{text}");
        assert!(!text.contains("jpeg-bytes"), "{text}");
        assert!(!text.contains("nothing was published"), "{text}");
        let Gate::Send(sent) = decide(&d, true, false).unwrap() else {
            panic!("a post with a site file is ready to send");
        };
        assert!(sent.attachments.iter().any(|a| a.name == "photo.jpg" && a.on_site));
        assert!(sent.attachments.iter().any(|a| a.name == "notes.pdf" && a.on_site));
        assert!(sent.attachments.iter().any(|a| a.name == "notes.txt" && !a.on_site));
        let frames = serde_json::to_string(&liveview_frames(&sent)).unwrap();
        assert!(!frames.contains("allow_upload") && !frames.contains("jpeg-bytes"), "{frames}");
        d.attachments = vec![bad.clone()];
        let notes = prepare(&d).unwrap();
        assert!(!notes.attachments[0].on_site, "a text file stays local");
        assert_eq!(notes.attachments[0].kind, AttachmentKind::File);
        d.site_attachments = Some(vec![0]);
        let refused = prepare(&d).unwrap_err().message;
        assert!(refused.contains("notes.txt"), "{refused}");
        assert!(refused.contains("image/*") && refused.contains("video/*"), "{refused}");
        d.site_attachments = None;
        d.attachments = vec![dir.join("missing.png")];
        assert!(prepare(&d).unwrap_err().message.contains("not found"));
        d.attachments = vec![dir.clone()];
        assert!(prepare(&d).unwrap_err().message.contains("not a file"));
        for ext in ["avif", "gif", "jpg", "jpeg", "png", "tif", "tiff", "webp", "mkv", "mov", "mp4", "webm"] {
            let path = dir.join(format!("sample.{ext}"));
            std::fs::write(&path, b"x").unwrap();
            assert_eq!(plan_attachment(&path).unwrap().kind, AttachmentKind::Media, "{ext}");
        }
        let upper = dir.join("PHOTO.JPG");
        std::fs::write(&upper, b"j").unwrap();
        assert_eq!(plan_attachment(&upper).unwrap().kind, AttachmentKind::Media);
        let pdf_path = dir.join("sample.pdf");
        std::fs::write(&pdf_path, b"%PDF").unwrap();
        let planned_pdf = plan_attachment(&pdf_path).unwrap();
        assert_eq!(planned_pdf.kind, AttachmentKind::File);
        assert!(planned_pdf.on_site);
        let empty = dir.join("empty.jpg");
        std::fs::write(&empty, b"").unwrap();
        d.site_attachments = None;
        d.attachments = vec![empty];
        let empty_err = decide(&d, true, false).unwrap_err().message;
        assert!(empty_err.contains("empty") && empty_err.contains("nothing was sent"), "{empty_err}");
        use clap::Parser;
        let parsed = Args::try_parse_from(["x", "--new-post", "Title", "--attachment", jpg.to_str().unwrap(), "--dry-run"]).unwrap();
        assert!(cli_requested(&parsed));
        assert_eq!(parsed.attachment.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn more_than_four_files_default_to_the_first_four_on_the_site_copy() {
        let dir = std::env::temp_dir().join(format!("acp-site-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let names = ["one.jpg", "two.png", "three.gif", "four.webp", "notes.pdf", "extra.png"];
        let paths: Vec<PathBuf> = names.iter().map(|n| {
            let p = dir.join(n);
            std::fs::write(&p, b"x").unwrap();
            p
        }).collect();
        let mut d = draft();
        d.attachments = paths.clone();
        let plan = prepare(&d).unwrap();
        assert!(plan.attachments.iter().take(4).all(|a| a.on_site));
        assert!(plan.attachments.iter().skip(4).all(|a| !a.on_site));
        let note = plan.site_notice.clone().unwrap();
        let kinds = crate::config::site_attachment_kinds();
        assert_eq!(note, format!("site copy keeps the first 4 of 6 files the site copy can take (Red Star allows 4: {kinds}). Left off the site: 5 notes.pdf, 6 extra.png"));
        let text = shown(decide(&d, false, true));
        assert!(text.contains(&note), "{text}");
        assert!(text.contains("local-only file notes.pdf"), "{text}");
        assert!(text.contains("site media one.jpg"), "{text}");
        assert!(!text.contains("nothing was published"), "{text}");
        assert!(text.contains("allow_upload"), "{text}");
        let Gate::Send(sent) = decide(&d, true, false).unwrap() else {
            panic!("the site copy is ready to send");
        };
        assert!(sent.attachments.iter().any(|a| a.name == "one.jpg" && a.on_site));
        assert!(sent.attachments.iter().any(|a| a.name == "notes.pdf" && !a.on_site));
        assert!(sent.site_notice.as_deref().is_some_and(|n| n.contains("5 notes.pdf")));
        let frames = serde_json::to_string(&liveview_frames(&plan)).unwrap();
        assert!(!frames.contains("notes.pdf"), "{frames}");
        d.attachments.truncate(4);
        let few = prepare(&d).unwrap();
        assert!(few.attachments.iter().all(|a| a.on_site));
        assert!(few.site_notice.is_none());
        assert!(shown(decide(&d, false, true)).contains("attachments (sent after confirm)"));
        assert!(shown(decide(&d, false, true)).contains("allow_upload"));
        d.attachments = paths.clone();
        d.site_attachments = Some(vec![0, 2, 4, 5]);
        let chosen_pdf = prepare(&d).unwrap();
        assert!(chosen_pdf.attachments[4].on_site, "a chosen pdf is uploaded");
        assert_eq!(chosen_pdf.attachments[4].kind.input_name(), "file_attachments");
        assert!(chosen_pdf.attachments[4].name.ends_with("notes.pdf"));
        d.site_attachments = Some(vec![0, 2, 3, 5]);
        let chosen = prepare(&d).unwrap();
        assert!(chosen.site_notice.is_none());
        assert_eq!(chosen.attachments.iter().map(|a| a.on_site).collect::<Vec<_>>(), vec![true, false, true, true, false, true]);
        d.site_attachments = Some(vec![9]);
        assert!(prepare(&d).unwrap_err().message.contains("not in the list"));
        use clap::Parser;
        let parsed = Args::try_parse_from([
            "x", "--new-post", "Title", "--dry-run", "--site-attachments", "1,3,5,6",
            "--attachment", paths[0].to_str().unwrap(),
        ]).unwrap();
        assert_eq!(parsed.site_attachments.as_deref(), Some(&[0usize, 2, 4, 5][..]));
        assert!(cli_requested(&parsed));
        let audio = dir.join("talk.mp3");
        std::fs::write(&audio, b"id3").unwrap();
        let cli = Args::try_parse_from([
            "x", "--new-post", "Title", "--dry-run",
            "--attachment", paths[0].to_str().unwrap(),
            "--attachment", audio.to_str().unwrap(),
            "--site-attachments", "2",
        ]).unwrap();
        d.attachments = cli.attachment.clone();
        d.site_attachments = cli.site_attachments.as_ref().map(|list| list.0.clone());
        let err = prepare(&d).unwrap_err().message;
        assert!(err.contains("talk.mp3"), "{err}");
        assert!(err.contains("image/*") && err.contains("video/*"), "{err}");
        d.site_attachments = None;
        let mut mixed = vec![audio];
        mixed.extend(paths.iter().filter(|p| p.extension().is_some_and(|e| e != "pdf")).cloned());
        d.attachments = mixed;
        let auto = prepare(&d).unwrap();
        assert!(!auto.attachments[0].on_site, "audio is not auto-selected");
        assert!(auto.attachments.iter().skip(1).take(4).all(|a| a.on_site));
        assert!(auto.attachments.iter().skip(5).all(|a| !a.on_site));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn root_reply_frames_match_the_captured_fixtures() {
        let validate: Value = serde_json::from_str(include_str!("../../tests/fixtures/site/reply_validate.json")).unwrap();
        let save: Value = serde_json::from_str(include_str!("../../tests/fixtures/site/reply_save.json")).unwrap();
        let join = include_str!("../../tests/fixtures/site/reply_join.json");
        assert!(join.contains("js-postForm-form"), "the join render mounts the composer");
        assert!(join.contains("phx-submit"));
        assert!(join.contains("phx-change"));
        assert!(join.contains("form[thread_id]"));
        assert!(join.contains("form[body_input]"));
        assert!(join.contains("<REDACTED:"));
        assert!(!join.contains("eyJ"), "a live JWT must not be in the fixture");
        assert!(join.contains("Attach PDF"), "the composer has a separate PDF input");
        assert!(!join.to_ascii_lowercase().contains("audio/"));

        let thread = "AaFV7IhwdC-iC6JuW97QMg";
        let body = "Something that I haven;t noted yet, is that I'm building out a core set of crates in Rust, then building on top of those. These crates are made so that other's can build on top of them.\n\nI personally plan on building a Flutter based application to tap into the network. This will be basically a pretty wrapper around the Rust crates via FFI.";
        let mut d = draft();
        d.title.clear();
        d.thread = Some(thread.into());
        d.body = body.into();
        let plan = prepare(&d).unwrap();
        assert_eq!(plan.cid, 2);
        assert_eq!(plan.page, format!("/rs/post/{thread}"));
        let frames = liveview_frames(&plan);
        assert_eq!(frames.len(), 2);

        let captured_validate = &validate["frames"][2]["frame"][4];
        let captured_save = &save["frames"][0]["frame"][4];
        let captured_value = captured_save["value"].as_str().unwrap();
        assert_eq!(captured_validate["value"].as_str().unwrap(), captured_value);
        assert_eq!(frames[0][4]["type"], "form");
        assert_eq!(frames[0][4]["event"], "validate");
        assert_eq!(frames[0][4]["meta"], captured_validate["meta"]);
        assert_eq!(frames[0][4]["uploads"], json!({}));
        assert_eq!(frames[0][4]["cid"], 2);
        assert_eq!(frames[0][4]["value"].as_str().unwrap(), captured_value);
        assert_eq!(frames[1][4]["event"], "save");
        assert_eq!(frames[1][4]["meta"], json!({}));
        assert!(frames[1][4].get("uploads").is_none());
        assert!(captured_save.get("uploads").is_none());
        assert_eq!(frames[1][4]["cid"], 2);
        assert_eq!(frames[1][4]["value"].as_str().unwrap(), captured_value);
        assert_eq!(plan.body, validate["frames"][2]["body_input_parsed"]);

        let trailing = &save["frames"][4];
        assert_eq!(trailing["frame"][4]["event"], "validate");
        assert!(trailing["note"].as_str().unwrap().contains("after save"));
        let unmatched = &save["frames"][5]["frame"][4];
        assert_eq!(unmatched["status"], "error");
        assert_eq!(unmatched["response"]["reason"], "unmatched topic");
        assert_eq!(save["frames"][2]["frame"][4]["e"][0][0], "thread:insert");
        assert_eq!(save["frames"][3]["frame"][4]["e"][0][0], "saving");
        assert_eq!(save["frames"][6]["frame"][3], "live_redirect");
        assert_eq!(save["frames"][6]["frame"][4]["kind"], "replace");

        let text = shown(decide(&d, false, true));
        assert!(text.contains("dry-run: no connection opened"), "{text}");
        assert!(text.contains("form[thread_id]="));
        assert!(text.contains("no validate is sent after save"), "{text}");
        assert!(!text.contains("https://acp.us"), "{text}");
        assert!(!text.contains("<REDACTED:"), "{text}");
        let Gate::Send(plan) = decide(&d, true, false).unwrap() else {
            panic!("a text reply is ready to send");
        };
        let payloads = event_payloads(&plan);
        assert_eq!(payloads.len(), 2);
        assert_eq!(payloads[0]["value"].as_str().unwrap(), captured_value);
        assert_eq!(payloads[1]["value"].as_str().unwrap(), captured_value);
        assert_eq!(payloads[1]["event"], "save");
    }

    #[test]
    fn capture_reply_does_not_describe_a_sent_frame() {
        let msg = capture_stub_message("T1");
        assert!(msg.contains("did not open a connection"));
        assert!(msg.contains("did not copy session.json"));
        assert!(msg.contains("does not send"));
        assert!(msg.contains("text posts and replies are published"));
        assert!(!msg.contains("nothing was published"));
    }

    #[test]
    fn a_text_reply_reads_the_captured_id_and_does_not_retry() {
        let save: Value = serde_json::from_str(include_str!("../../tests/fixtures/site/reply_save.json")).unwrap();
        let frames: Vec<Value> = save["frames"].as_array().unwrap().iter().map(|f| f["frame"].clone()).collect();
        let mut d = draft();
        d.title.clear();
        d.thread = Some("AaFV7IhwdC-iC6JuW97QMg".into());
        d.body = "hello".into();
        let Gate::Send(plan) = decide(&d, true, false).unwrap() else { panic!("text reply") };
        let payloads = event_payloads(&plan);
        assert_eq!(payloads.len(), 2, "validate then save, and no third frame");
        assert_eq!(payloads[0]["event"], "validate");
        assert_eq!(payloads[1]["event"], "save");

        let first = after_reply(&payloads, 0, &[]).unwrap().expect("validate");
        assert_eq!(first["event"], "validate");
        let ok = json!({"status": "ok", "response": {"diff": {}}});
        let second = after_reply(&payloads, 1, &[ok.clone()]).unwrap().expect("save");
        assert_eq!(second["event"], "save");
        assert!(after_reply(&payloads, 2, &[ok.clone(), ok.clone()]).unwrap().is_none());

        let rejected = json!({"status": "error", "response": {"reason": "_csrf_token=SECRETSECRETSECRETSECRETSECRET12"}});
        let stopped = after_reply(&payloads, 1, &[rejected]).unwrap_err();
        assert!(stopped.message.contains("the site rejected the post"), "{}", stopped.message);
        assert!(stopped.message.contains("Nothing was retried"), "{}", stopped.message);
        assert!(!stopped.message.contains("SECRETSECRET"), "{}", stopped.message);

        let save_err = json!(["1", "2", "lv:x", "phx_reply", {"status": "error", "response": {"reason": "bad body"}}]);
        let no_third = after_reply(&payloads, 2, &[ok.clone(), save_err]).unwrap_err();
        assert!(no_third.message.contains("bad body"), "{}", no_third.message);
        assert!(no_third.message.contains("Nothing was retried"), "{}", no_third.message);

        let missing = interpret(&[ok.clone(), ok], "https://acp.us").unwrap_err();
        assert!(missing.message.contains("did not return a post id"), "{}", missing.message);
        assert!(missing.message.contains("Nothing was retried"), "{}", missing.message);
        assert!(missing.message.contains("Check the site"), "{}", missing.message);

        let seen = [frames[1].clone(), frames[2].clone(), frames[3].clone(), frames[6].clone()];
        assert!(after_reply(&payloads, 2, &seen).unwrap().is_none(), "the id frames are not another push");
        let published = interpret(&seen, "https://acp.us").unwrap();
        assert_eq!(published.id, "AaFWh52wdleDeaWo1se9AQ");
        assert_eq!(published.url, "https://acp.us/rs/post/AaFV7IhwdC-iC6JuW97QMg/AaFWh52wdleDeaWo1se9AQ");
        let shown_ok = format!("{published:?}");
        assert!(!shown_ok.contains("REDACTED"), "{shown_ok}");
        assert!(!shown_ok.contains("flash"), "{shown_ok}");

        d.in_reply_to = Some("COMMENT1".into());
        let Gate::Send(comment) = decide(&d, true, false).unwrap() else { panic!("comment reply") };
        assert_eq!(comment.body["content"][0]["attrs"]["postId"], "COMMENT1");
        assert_eq!(comment.body["content"][0]["attrs"]["threadId"], "AaFV7IhwdC-iC6JuW97QMg");
        assert_eq!(event_payloads(&comment).len(), 2);
    }

    const FORM: &str = r#"
        <html><head>
          <meta name="csrf-token" content="page-token">
          <script src="/assets/app-abc123.js"></script>
        </head><body>
          <form id="js-postForm-form" phx-submit="save" phx-change="validate" phx-target="2">
            <input type="hidden" name="_csrf_token" value="SECRETSECRETSECRETSECRETSECRET12">
            <input type="hidden" name="form[thread_id]" value="from-the-page">
          </form>
        </body></html>
    "#;
}
