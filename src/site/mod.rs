//! Write core (3.4.0): New post and Reply.
//!
//! The safety layer, the Markdown to ProseMirror writer, and the documented
//! form frames live here. Publishing does not open a connection. The P0
//! guided capture has not confirmed the submit payload, so validate and save
//! stay behind [`write::CAPTURE_BLOCK`]. Attachments wait for 3.5.0. Vote,
//! subscribe, delete, edit, and repost are later minors and are not built.

mod prosemirror;
mod write;

pub use prosemirror::{BODY_CHAR_LIMIT, assemble, markdown_to_doc, plain_blocks, post_ref};
pub use write::{
    CAPTURE_BLOCK, Completion, Draft, Kind, NickHint, Probe, WriteError, WritePlan, bundle_changed, capture_stub_message,
    completions, decide, dry_run_text, liveview_frames, prepare, probe_html, rate_wait, refuse_live_write, writes_enabled,
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
}

/// Run a write command. Cancel prints `nothing sent` and returns Ok.
/// A confirmed publish returns the capture error and does not connect.
pub fn run_cli(args: &Args) -> Result<()> {
    if let Some(thread) = args.capture_reply.as_deref() {
        bail!("{}", capture_stub_message(thread.trim()));
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
        attachments: args.attachment.len(),
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
    match decide(&draft, confirmed, args.dry_run) {
        Ok(text) => {
            println!("{text}");
            Ok(())
        }
        Err(e) => bail!("{e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> Draft {
        Draft {
            title: "Strike".into(),
            body: "hello **world**".into(),
            thread: None,
            in_reply_to: None,
            quote: None,
            quote_post: None,
            attachments: 0,
            html: None,
            accept_bundle: false,
            stored_bundle: None,
            now_unix: 1_000,
            last_write_unix: None,
            writes_on: true,
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
        assert_eq!(names, ["form[body_input]", "form[thread_id]"]);
        let body = &plan.fields[0].1;
        assert!(body.contains("postRef") && body.contains("agreed"));
    }

    #[test]
    fn reply_to_the_thread_root_has_no_post_ref() {
        let mut d = draft();
        d.title.clear();
        d.thread = Some("T1".into());
        d.body = "hello".into();
        let plan = prepare(&d).unwrap();
        assert!(plan.body["content"].as_array().unwrap().iter().all(|n| n["type"] != "postRef"));
        assert!(plan.fields.iter().any(|(k, v)| k == "form[thread_id]" && v == "T1"));
    }

    #[test]
    fn dry_run_prints_frames_and_cancel_sends_nothing() {
        let d = draft();
        let text = decide(&d, false, true).unwrap();
        assert!(text.contains("dry-run: no connection opened"), "{text}");
        assert!(text.contains("\"event\": \"validate\""), "{text}");
        assert!(text.contains("\"event\": \"save\""), "{text}");
        assert!(text.contains("form%5Btitle%5D=Strike") || text.contains("form[title]"), "{text}");
        assert!(text.contains("_target"), "{text}");
        assert!(!text.contains("https://acp.us"), "{text}");
        assert_eq!(decide(&d, false, false).unwrap(), "nothing sent");
    }

    #[test]
    fn confirm_is_blocked_and_does_not_open_a_connection() {
        let err = decide(&draft(), true, false).unwrap_err();
        assert!(err.message.contains("nothing was published"), "{}", err.message);
        assert!(err.message.contains("dry-run: no connection opened"), "{}", err.message);
    }

    #[test]
    fn writes_off_attachments_bundle_and_rate_limit_block() {
        let mut d = draft();
        d.writes_on = false;
        assert!(prepare(&d).unwrap_err().message.contains("writes are off"));
        d.writes_on = true;
        d.attachments = 1;
        assert!(prepare(&d).unwrap_err().message.contains("3.5.0"));
        d.attachments = 0;
        d.html = Some(FORM.into());
        d.stored_bundle = Some("/assets/app-old.js".into());
        let err = decide(&d, true, false).unwrap_err();
        assert!(err.message.contains("bundle changed"), "{}", err.message);
        d.accept_bundle = true;
        let err = decide(&d, true, false).unwrap_err();
        assert!(err.message.contains("nothing was published"), "{}", err.message);
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
        assert!(plan.fields.iter().any(|(k, _)| k == "_csrf_token"));
        let shown = dry_run_text(&plan);
        assert!(!shown.contains("SECRETSECRETSECRETSECRETSECRET12"), "{shown}");
        assert!(shown.contains("[REDACTED]"), "{shown}");
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
        assert!(err.to_string().contains("nothing was published"));
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
    fn capture_reply_does_not_describe_a_sent_frame() {
        let msg = capture_stub_message("T1");
        assert!(msg.contains("did not open a connection"));
        assert!(msg.contains("did not copy session.json"));
        assert!(msg.contains("nothing was published"));
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
