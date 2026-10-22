//! Vote, subscribe, site bookmark, delete, edit, and quote repost.
//!
//! Cancel is the default. Delete and edit also ask for the word `yes`.
//! `--dry-run` prints the action and does not call a service. A confirmed
//! action calls the loaded capability. The site half of each write is the
//! service package. This module does not open a socket.

use acp_rs_plugin::{CapabilityId, DynService, ServiceId, SourceId, VoteChoice, WriteOutcome};

use crate::prelude::*;

/// The site does not allow a repost with no comment.
pub const PLAIN_REPOST_REFUSED: &str =
    "The site does not allow a plain repost. Add a quote with --body. Nothing was sent.";

/// Unsubscribe was not part of the captured subscribe.
pub const UNSUBSCRIBE_MISSING: &str = "unsubscribe was not captured. The missing frame is a subscribe hook with state \"0\". Nothing was sent.";

/// A repost needs the captured title field.
pub const REPOST_TITLE_REQUIRED: &str =
    "the captured repost save includes a title. Pass --repost-title. Nothing was sent.";

/// An edit cannot change attachments.
pub const ATTACHMENT_EDIT_REFUSAL: &str =
    "The site does not support changing attachments on an edit. Nothing was sent.";

/// One hour after `posted_at`.
pub const EDIT_WINDOW_SECS: i64 = 3600;

/// A reply edit does not take a quote flag.
pub const QUOTE_ON_EDIT: &str = "a reply edit takes the body only. A quote is not added. Nothing was sent.";

/// Writes are switched off.
pub const WRITES_OFF: &str = "writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent.";

/// One confirmed write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Vote state `1`, `0`, or `-1`.
    Vote {
        /// Post id.
        id: String,
        /// `1`, `0`, or `-1`.
        state: String,
    },
    /// Subscribe state `1`.
    Subscribe {
        /// Thread id.
        id: String,
    },
    /// Site bookmark state `1`.
    Bookmark {
        /// Post id.
        id: String,
    },
    /// Delete. The local row stays.
    Delete {
        /// Post id.
        id: String,
    },
    /// Reply body, or a top-level title and body.
    Edit {
        /// Post id.
        id: String,
        /// `--edit-title`, when set.
        title: Option<String>,
        /// Markdown body. Empty when only the title changes.
        body: String,
        /// How the hour was described.
        hour: &'static str,
    },
    /// Quote repost.
    Repost {
        /// Post id.
        id: String,
        /// New title.
        title: String,
        /// Comment.
        body: String,
    },
}

/// These flags are the interaction commands.
pub fn cli_requested(args: &Args) -> bool {
    args.vote.is_some()
        || args.subscribe.is_some()
        || args.site_bookmark.is_some()
        || args.delete.is_some()
        || args.edit.is_some()
        || args.repost.is_some()
}

/// Validate `args`. This does not prompt and does not call a service.
pub fn check(args: &Args, now: i64) -> Result<Kind> {
    if !crate::prefs::writes_enabled() {
        bail!("{WRITES_OFF}");
    }
    if let Some(id) = args.vote.as_deref() {
        let state = args.vote_state.as_deref().unwrap_or("1");
        if !matches!(state, "1" | "0" | "-1") {
            bail!("vote state must be 1, 0, or -1");
        }
        return Ok(Kind::Vote { id: clean_id(id, "vote")?, state: state.to_string() });
    }
    if let Some(id) = args.subscribe.as_deref() {
        let state = args.subscribe_state.as_deref().unwrap_or("1");
        if state == "0" {
            bail!("{UNSUBSCRIBE_MISSING}");
        }
        if state != "1" {
            bail!("subscribe state must be 1 or 0");
        }
        return Ok(Kind::Subscribe { id: clean_id(id, "subscribe")? });
    }
    if let Some(id) = args.site_bookmark.as_deref() {
        return Ok(Kind::Bookmark { id: clean_id(id, "bookmark")? });
    }
    if let Some(id) = args.delete.as_deref() {
        return Ok(Kind::Delete { id: clean_id(id, "delete")? });
    }
    if let Some(id) = args.edit.as_deref() {
        if args.quote.is_some() {
            bail!("{QUOTE_ON_EDIT}");
        }
        if !args.attachment.is_empty() {
            bail!("{ATTACHMENT_EDIT_REFUSAL}");
        }
        let body = read_body(args)?;
        let title = args.edit_title.clone().filter(|s| !s.trim().is_empty());
        if title.is_none() && body.trim().is_empty() {
            bail!("an edit needs --body or --body-file, or --edit-title. Nothing was sent.");
        }
        let hour = hour_line(args.posted_at, now)?;
        return Ok(Kind::Edit { id: clean_id(id, "edit")?, title, body, hour });
    }
    if let Some(id) = args.repost.as_deref() {
        let body = read_body(args)?;
        if body.trim().is_empty() {
            bail!("{PLAIN_REPOST_REFUSED}");
        }
        let title = args.repost_title.as_deref().unwrap_or("").trim();
        if title.is_empty() {
            bail!("{REPOST_TITLE_REQUIRED}");
        }
        return Ok(Kind::Repost {
            id: clean_id(id, "repost")?,
            title: title.to_string(),
            body,
        });
    }
    bail!("a write needs a vote, subscribe, bookmark, delete, edit, or repost");
}

/// Text for `--dry-run`. No socket and no service call.
pub fn dry_text(kind: &Kind) -> String {
    let intro = match kind {
        Kind::Vote { id, state } => {
            format!("vote {id} state {state}\nThe reply shows score, vote, and upvote_percentage.")
        }
        Kind::Subscribe { id } => format!("subscribe {id} state 1\nThe reply shows subscribed true."),
        Kind::Bookmark { id } => format!(
            "bookmark {id} state 1\nThe reply shows bookmarked true and bookmarks_count. Clearing a bookmark (state \"0\") is not sent."
        ),
        Kind::Delete { id } => format!(
            "delete {id}\nCancel sends nothing. A confirmed delete calls the loaded service. The local copy stays, with a tombstone."
        ),
        Kind::Edit { id, title, body, hour } => {
            format!("edit {id}\ntitle: {}\n{hour}\n{body}", title.as_deref().unwrap_or(""))
        }
        Kind::Repost { id, title, body } => {
            format!("repost {id}\ntitle: {title}\n{body}\nThe site does not allow a plain repost.")
        }
    };
    format!("dry-run: no connection opened\n{intro}\n")
}

/// Capability this write calls.
pub fn capability(kind: &Kind) -> CapabilityId {
    match kind {
        Kind::Vote { .. } => CapabilityId::Vote,
        Kind::Subscribe { .. } => CapabilityId::Subscribe,
        Kind::Bookmark { .. } => CapabilityId::Bookmark,
        Kind::Delete { .. } => CapabilityId::Delete,
        Kind::Edit { .. } => CapabilityId::Edit,
        Kind::Repost { .. } => CapabilityId::Repost,
    }
}

/// Ask, then call. A dry-run returns before the prompt.
pub async fn run(args: &Args) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let kind = check(args, now)?;
    if args.dry_run {
        println!("{}", dry_text(&kind));
        return Ok(());
    }
    if !confirmed(args, &kind)? {
        println!("nothing sent");
        return Ok(());
    }
    let cap = capability(&kind);
    let Some(live) = crate::plugins::live(cap) else {
        bail!("{}", crate::service::SERVICE_ABSENT);
    };
    let text = call(live.get(), &kind).await?;
    match &kind {
        Kind::Delete { id } => {
            let db = home()?.join(DB_REL_PATH);
            let c = open_db(&db)?;
            let at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            crate::storage::db::note_tombstone(&c, id, &at)?;
        }
        Kind::Bookmark { id } => {
            crate::host_api::CoreHost::shared()
                .bookmark_post(live.get().id(), id, true)
                .map_err(|err| anyhow!(err.message))?;
        }
        _ => {}
    }
    println!("{text}");
    Ok(())
}

/// Call `service`. The host records a tombstone or a local bookmark after this returns.
pub async fn call(service: &dyn DynService, kind: &Kind) -> Result<String> {
    let native = match kind {
        Kind::Vote { id, .. }
        | Kind::Subscribe { id }
        | Kind::Bookmark { id }
        | Kind::Delete { id }
        | Kind::Edit { id, .. }
        | Kind::Repost { id, .. } => id.clone(),
    };
    let source = SourceId::new(ServiceId::new(service.id()), native);
    match kind {
        Kind::Vote { state, .. } => {
            let choice = match state.as_str() {
                "1" => VoteChoice::Up,
                "0" => VoteChoice::Clear,
                _ => VoteChoice::Down,
            };
            service.vote(&source, choice).await.map_err(fail)?;
            Ok(format!("voted {} state {state}", source.native))
        }
        Kind::Subscribe { .. } => {
            service.subscribe(&source, true).await.map_err(fail)?;
            Ok(format!("subscribed {}", source.native))
        }
        Kind::Bookmark { .. } => {
            service.bookmark(&source, true).await.map_err(fail)?;
            Ok(format!("bookmarked {}", source.native))
        }
        Kind::Delete { .. } => {
            service.delete(&source).await.map_err(fail)?;
            Ok(format!("deleted {}\nthe local copy stays, with a tombstone", source.native))
        }
        Kind::Edit { body, .. } => match service.edit(&source, body).await.map_err(fail)? {
            WriteOutcome::NativeId(id) => Ok(format!("edited {}", id.native)),
            WriteOutcome::NotReady(_) => Ok("nothing sent".into()),
        },
        Kind::Repost { body, .. } => match service.repost(&source, Some(body)).await.map_err(fail)? {
            WriteOutcome::NativeId(id) => Ok(format!("reposted {}", id.native)),
            WriteOutcome::NotReady(_) => Ok("nothing sent".into()),
        },
    }
}

fn fail(err: acp_rs_plugin::ServiceError) -> anyhow::Error {
    anyhow!(err.to_string())
}

fn clean_id(id: &str, what: &str) -> Result<String> {
    let id = id.trim();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
        bail!("a {what} needs a post id");
    }
    Ok(id.to_string())
}

fn read_body(args: &Args) -> Result<String> {
    match (&args.body, &args.body_file) {
        (Some(_), Some(_)) => bail!("pass --body or --body-file, not both"),
        (Some(text), None) => Ok(text.clone()),
        (None, Some(path)) => std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display())),
        (None, None) => Ok(String::new()),
    }
}

fn hour_line(posted_at: Option<i64>, now: i64) -> Result<&'static str> {
    let Some(at) = posted_at else {
        return Ok("hour not checked");
    };
    if now.saturating_sub(at) > EDIT_WINDOW_SECS {
        bail!("an edit stays open for one hour after the post. Nothing was sent.");
    }
    Ok("within one hour")
}

fn confirmed(args: &Args, kind: &Kind) -> Result<bool> {
    if args.yes {
        return Ok(true);
    }
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(false);
    }
    let prompt = match kind {
        Kind::Vote { .. } => "Vote? [y/N] ",
        Kind::Subscribe { .. } => "Subscribe? [y/N] ",
        Kind::Bookmark { .. } => "Bookmark? [y/N] ",
        Kind::Delete { .. } => "Delete? [y/N] ",
        Kind::Edit { .. } => "Edit? [y/N] ",
        Kind::Repost { .. } => "Repost? [y/N] ",
    };
    eprint!("{prompt}");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let yes = matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    if !yes {
        return Ok(false);
    }
    if matches!(kind, Kind::Delete { .. } | Kind::Edit { .. }) {
        eprint!("Type yes: ");
        io::stderr().flush()?;
        let mut typed = String::new();
        io::stdin().read_line(&mut typed)?;
        return Ok(typed.trim() == "yes");
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_rs_plugin::{Availability, Field, RuntimeCtx};
    use clap::Parser;
    use std::sync::Mutex;

    fn args(argv: &[&str]) -> Args {
        Args::try_parse_from(argv).unwrap()
    }

    #[test]
    fn dry_run_names_the_action_and_a_plain_repost_is_refused() {
        let vote = check(&args(&["x", "--vote", "POST", "--vote-state=-1", "--dry-run"]), 0).unwrap();
        let dry = dry_text(&vote);
        assert!(dry.starts_with("dry-run: no connection opened\n"), "{dry}");
        assert!(dry.contains("vote POST state -1"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        assert!(!dry.contains("phx_"), "{dry}");
        let mark = check(&args(&["x", "--site-bookmark", "POST", "--dry-run"]), 0).unwrap();
        assert!(dry_text(&mark).contains("bookmark POST state 1"), "{mark:?}");
        assert!(dry_text(&mark).contains("state \"0\""));
        let gone = check(&args(&["x", "--subscribe", "THREAD", "--subscribe-state", "0", "--dry-run"]), 0);
        assert!(gone.unwrap_err().to_string().contains("state \"0\""));
        let plain = check(&args(&["x", "--repost", "POST", "--dry-run"]), 0);
        assert!(plain.unwrap_err().to_string().contains("does not allow a plain repost"));
        let titled = check(
            &args(&["x", "--repost", "POST", "--repost-title", "Hello", "--body", "a quote", "--dry-run"]),
            0,
        )
        .unwrap();
        let text = dry_text(&titled);
        assert!(text.contains("repost POST"), "{text}");
        assert!(text.contains("title: Hello"), "{text}");
        assert!(text.contains("does not allow a plain repost"), "{text}");
    }

    #[test]
    fn the_hour_closes_and_an_edit_names_the_window() {
        let closed = check(&args(&["x", "--edit", "POST", "--body", "hi", "--posted-at", "0", "--dry-run"]), 3601);
        assert!(closed.unwrap_err().to_string().contains("one hour"));
        let open = check(&args(&["x", "--edit", "POST", "--body", "hi", "--posted-at", "10", "--dry-run"]), 20).unwrap();
        assert!(dry_text(&open).contains("within one hour"));
        let skipped = check(&args(&["x", "--edit", "POST", "--edit-title", "Title", "--dry-run"]), 0).unwrap();
        assert!(dry_text(&skipped).contains("hour not checked"));
        assert!(check(&args(&["x", "--edit", "POST", "--body", "hi", "--quote", "no", "--dry-run"]), 0).is_err());
    }

    struct Fake {
        log: Mutex<Vec<String>>,
    }

    impl DynService for Fake {
        fn id(&self) -> &'static str {
            "redstar"
        }
        fn display_name(&self) -> &'static str {
            "Fake"
        }
        fn implemented(&self) -> &'static [CapabilityId] {
            &[
                CapabilityId::Vote,
                CapabilityId::Subscribe,
                CapabilityId::Bookmark,
                CapabilityId::Delete,
                CapabilityId::Edit,
                CapabilityId::Repost,
            ]
        }
        fn schema(&self) -> &'static [Field] {
            &[]
        }
        fn availability(&self, _cap: CapabilityId, _ctx: &RuntimeCtx) -> Availability {
            Availability::Ready
        }
        fn vote<'a>(&'a self, item: &'a SourceId, choice: VoteChoice) -> acp_rs_plugin::CapFut<'a, ()> {
            self.log.lock().unwrap().push(format!("vote {} {choice:?}", item.native));
            Box::pin(async { Ok(()) })
        }
        fn subscribe<'a>(&'a self, item: &'a SourceId, enabled: bool) -> acp_rs_plugin::CapFut<'a, ()> {
            self.log.lock().unwrap().push(format!("subscribe {} {enabled}", item.native));
            Box::pin(async { Ok(()) })
        }
        fn bookmark<'a>(&'a self, item: &'a SourceId, on: bool) -> acp_rs_plugin::CapFut<'a, ()> {
            self.log.lock().unwrap().push(format!("bookmark {} {on}", item.native));
            Box::pin(async { Ok(()) })
        }
        fn delete<'a>(&'a self, item: &'a SourceId) -> acp_rs_plugin::CapFut<'a, ()> {
            self.log.lock().unwrap().push(format!("delete {}", item.native));
            Box::pin(async { Ok(()) })
        }
        fn edit<'a>(&'a self, item: &'a SourceId, body: &'a str) -> acp_rs_plugin::CapFut<'a, WriteOutcome> {
            self.log.lock().unwrap().push(format!("edit {} {body}", item.native));
            let id = item.clone();
            Box::pin(async move { Ok(WriteOutcome::NativeId(id)) })
        }
        fn repost<'a>(&'a self, item: &'a SourceId, quote: Option<&'a str>) -> acp_rs_plugin::CapFut<'a, WriteOutcome> {
            self.log.lock().unwrap().push(format!("repost {} {}", item.native, quote.unwrap_or("")));
            let id = item.clone();
            Box::pin(async move { Ok(WriteOutcome::NativeId(id)) })
        }
    }

    #[tokio::test]
    async fn a_fake_service_receives_the_call_and_a_tombstone_is_local() {
        let fake = Fake { log: Mutex::new(Vec::new()) };
        let kind = Kind::Vote { id: "POST".into(), state: "-1".into() };
        let text = call(&fake, &kind).await.unwrap();
        assert_eq!(text, "voted POST state -1");
        assert_eq!(fake.log.lock().unwrap().as_slice(), ["vote POST Down"]);
        let c = Connection::open_in_memory().unwrap();
        crate::storage::db::note_tombstone(&c, "POST", "2026-10-22T00:00:00Z").unwrap();
        let at: String = c
            .query_row("SELECT deleted_at FROM post_tombstones WHERE post_id = 'POST'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(at, "2026-10-22T00:00:00Z");
        let again = crate::storage::db::note_tombstone(&c, "POST", "2026-10-22T01:00:00Z");
        assert!(again.is_ok());
    }
}
