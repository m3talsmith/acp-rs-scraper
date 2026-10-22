//! Scrape, sync, login, and compose flags call the loaded service.
//!
//! This binary is the headless CLI. The terminal UI starts the same flags in a
//! child process. A service that implements the capability runs the job. Until
//! one is loaded, those commands are not in this binary. The service does not
//! ship its own command line.

use acp_rs_plugin::{
    Budget, CapabilityId, Cursor, Draft, DynService, FilledFields, Host, RouteKind, ServiceError,
    ServiceId, SourceId, Target, WriteOutcome,
};

use crate::prelude::*;

/// The loaded service implements `cap`.
pub fn handles(cap: CapabilityId) -> bool {
    service(cap).is_some()
}

fn service(cap: CapabilityId) -> Option<crate::plugins::Live> {
    crate::plugins::live(cap)
}

fn fail(err: ServiceError) -> anyhow::Error {
    anyhow!(err.to_string())
}

/// Log in through the loaded authenticator and seal what it returns.
pub async fn login(host: &dyn Host) -> Result<()> {
    let live = service(CapabilityId::Authenticator).context("no authenticator is loaded")?;
    let mut fields = FilledFields::new();
    fields.insert("email", host.email().unwrap_or_default());
    let sealed = live
        .get()
        .authenticate(RouteKind::Session, &fields)
        .await
        .map_err(fail)?;
    if !sealed.field.is_empty() && !sealed.material.is_empty() {
        host.store_secret(&sealed.field, &sealed.material)
            .map_err(|err| anyhow!(err.message))?;
    }
    info!("logged in");
    Ok(())
}

/// Read flags (`--nick`, `--sync`, `--sync-bookmarks`, `--scrape-all`, and the site feeds).
pub async fn read_flags(args: &Args) -> Result<()> {
    let live = service(CapabilityId::Reader).context("no reader is loaded")?;
    let host = crate::host_api::CoreHost::shared();
    let mut targets = Vec::new();
    if args.sync_bookmarks || (args.sync && !args.no_bookmarks) {
        targets.push(Target::Bookmarks);
    }
    if args.scrape_all {
        targets.push(Target::Site);
    }
    if let Some(period) = args.top {
        targets.push(Target::Playlist {
            id: format!("top:{}", period.as_str()),
        });
    }
    if args.eb {
        targets.push(Target::Playlist { id: "eb".into() });
    }
    if args.explore || args.explore_latest {
        targets.push(Target::Playlist {
            id: if args.explore_latest { "explore:latest".into() } else { "explore".into() },
        });
    }
    if let Some(query) = &args.site_search {
        targets.push(Target::Search { query: query.clone() });
    }
    if let Some(tag) = &args.tag {
        targets.push(Target::Playlist { id: format!("tag:{tag}") });
    }
    if args.refetch_truncated {
        targets.push(Target::Playlist { id: "refetch-truncated".into() });
    }
    for nick in args.nick_list()? {
        targets.push(Target::Account { handle: nick });
    }
    if targets.is_empty() {
        info!("sync: no nicks given, nothing to scrape");
        return Ok(());
    }
    let pages = read_with(live.get(), Some(host.as_ref()), &targets).await?;
    info!("service {}: {pages} pages", live.get().id());
    Ok(())
}

/// Resolve and page each target. Pages are stored through `host` when it is set.
pub async fn read_with(service: &dyn DynService, host: Option<&dyn Host>, targets: &[Target]) -> Result<usize> {
    let mut pages = 0;
    for target in targets {
        let resolved = service.resolve(target).await.map_err(fail)?;
        let mut cursor = Cursor::default();
        let mut budget = Budget::unlimited();
        let mut seen = std::collections::HashSet::new();
        loop {
            let page = service
                .page(&resolved, &cursor, None, &mut budget)
                .await
                .map_err(fail)?;
            pages += 1;
            if let Some(host) = host
                && (!page.items.is_empty() || !page.accounts.is_empty() || !page.segments.is_empty())
            {
                host.save_page(&page).map_err(|err| anyhow!(err.message))?;
            }
            let Some(next) = page.next else { break };
            if !seen.insert(next.as_str().to_string()) {
                break;
            }
            cursor = next;
            if pages > 10_000 {
                bail!("the service did not finish the read");
            }
        }
    }
    Ok(pages)
}

/// `--new-post` / `--reply` through the loaded service.
///
/// `None` means this binary's site modules still own the command. A dry-run
/// stays there so the printed frames do not change.
pub async fn compose(args: &Args) -> Result<Option<String>> {
    if args.dry_run || (args.new_post.is_none() && args.reply.is_none()) {
        return Ok(None);
    }
    let reply = args.reply.is_some();
    let cap = if reply { CapabilityId::Reply } else { CapabilityId::Post };
    let Some(live) = service(cap) else { return Ok(None) };
    if !publish_confirmed(args) {
        println!("nothing sent");
        return Ok(Some(String::new()));
    }
    let body = match (&args.body, &args.body_file) {
        (Some(_), Some(_)) => bail!("pass --body or --body-file, not both"),
        (Some(text), None) => text.clone(),
        (None, Some(path)) => std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
        (None, None) => String::new(),
    };
    let mut draft = Draft::text(body);
    draft.title = args.new_post.clone().unwrap_or_default();
    if let Some(thread) = &args.reply {
        draft.in_reply_to = Some(SourceId::new(ServiceId::new(live.get().id()), thread.clone()));
    }
    if let Some(parent) = &args.in_reply_to {
        draft.in_reply_to = Some(SourceId::new(ServiceId::new(live.get().id()), parent.clone()));
    }
    let outcome = if reply {
        live.get().reply(&draft).await.map_err(fail)?
    } else {
        live.get().publish(&draft).await.map_err(fail)?
    };
    Ok(Some(outcome_text(outcome)))
}

/// Publish `draft` through the loaded service.
pub async fn publish_draft(draft: &Draft) -> Result<String> {
    let reply = draft.in_reply_to.is_some();
    let cap = if reply { CapabilityId::Reply } else { CapabilityId::Post };
    let live = service(cap).context("no writer is loaded")?;
    let outcome = if reply {
        live.get().reply(draft).await.map_err(fail)?
    } else {
        live.get().publish(draft).await.map_err(fail)?
    };
    Ok(outcome_text(outcome))
}

fn outcome_text(outcome: WriteOutcome) -> String {
    match outcome {
        WriteOutcome::NativeId(id) => id.native,
        WriteOutcome::NotReady(_) => "nothing sent".into(),
    }
}

fn publish_confirmed(args: &Args) -> bool {
    if args.yes {
        return true;
    }
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        eprint!("Publish? [y/N] ");
        let _ = std::io::Write::flush(&mut std::io::stderr());
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        return matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_rs_plugin::{Availability, Field, Page, Resolved, RuntimeCtx, SealedMaterial};
    use std::sync::Mutex;

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
                CapabilityId::Authenticator,
                CapabilityId::Reader,
                CapabilityId::Post,
                CapabilityId::Reply,
            ]
        }
        fn schema(&self) -> &'static [Field] {
            &[]
        }
        fn availability(&self, _cap: CapabilityId, _ctx: &RuntimeCtx) -> Availability {
            Availability::Ready
        }
        fn authenticate<'a>(
            &'a self,
            _route: RouteKind,
            fields: &'a FilledFields,
        ) -> acp_rs_plugin::CapFut<'a, SealedMaterial> {
            self.log.lock().unwrap().push(format!("login {}", fields.get("email").unwrap_or("")));
            Box::pin(async { Ok(SealedMaterial::new("session", b"sealed".to_vec())) })
        }
        fn resolve<'a>(&'a self, target: &'a Target) -> acp_rs_plugin::CapFut<'a, Resolved> {
            let name = match target {
                Target::Account { handle } => format!("account:{handle}"),
                Target::Bookmarks => "bookmarks".into(),
                Target::Site => "site".into(),
                Target::Search { query } => format!("search:{query}"),
                Target::Playlist { id } => format!("list:{id}"),
                Target::Item { id } | Target::Conversation { id } => id.native.clone(),
            };
            self.log.lock().unwrap().push(format!("resolve {name}"));
            let source = SourceId::new(ServiceId::new("redstar"), name);
            let target = target.clone();
            Box::pin(async move { Ok(Resolved { target, source_id: source }) })
        }
        fn page<'a>(
            &'a self,
            resolved: &'a Resolved,
            cursor: &'a Cursor,
            _stop_at: Option<&'a str>,
            _budget: &'a mut Budget,
        ) -> acp_rs_plugin::CapFut<'a, Page> {
            self.log.lock().unwrap().push(format!("page {} {}", resolved.source_id.native, cursor.as_str()));
            Box::pin(async { Ok(Page::empty()) })
        }
        fn publish<'a>(&'a self, draft: &'a Draft) -> acp_rs_plugin::CapFut<'a, WriteOutcome> {
            self.log.lock().unwrap().push(format!("publish {}", draft.title));
            Box::pin(async {
                Ok(WriteOutcome::NativeId(SourceId::new(ServiceId::new("redstar"), "new-1")))
            })
        }
        fn reply<'a>(&'a self, draft: &'a Draft) -> acp_rs_plugin::CapFut<'a, WriteOutcome> {
            let parent = draft.in_reply_to.as_ref().map(|id| id.native.clone()).unwrap_or_default();
            self.log.lock().unwrap().push(format!("reply {parent}"));
            Box::pin(async {
                Ok(WriteOutcome::NativeId(SourceId::new(ServiceId::new("redstar"), "reply-1")))
            })
        }
    }

    fn block<F: std::future::Future>(future: F) -> F::Output {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(future)
    }

    #[test]
    fn linked_x_and_youtube_keep_reads_gated() {
        let reg = crate::service::Registry::builtins();
        let ids: Vec<_> = reg.services().iter().map(|service| service.id()).collect();
        assert_eq!(ids, ["x", "youtube"]);
        let on = acp_rs_plugin::RuntimeCtx::ready();
        for service in reg.services() {
            assert_ne!(service.id(), "redstar");
            assert_eq!(
                service.availability(CapabilityId::Reader, &on),
                acp_rs_plugin::Availability::Gated
            );
        }
    }

    #[test]
    fn flags_dispatch_to_the_loaded_service() {
        let fake = Fake { log: Mutex::new(Vec::new()) };
        let targets = [
            Target::Bookmarks,
            Target::Site,
            Target::Account { handle: "ada".into() },
        ];
        let pages = block(read_with(&fake, None, &targets)).unwrap();
        assert_eq!(pages, 3);
        let mut draft = Draft::text("hello");
        draft.title = "Hello".into();
        let id = block(async {
            fake.publish(&draft).await.unwrap()
        });
        assert!(matches!(id, WriteOutcome::NativeId(_)));
        let log = fake.log.lock().unwrap().clone();
        assert!(log.iter().any(|line| line == "resolve bookmarks"), "{log:?}");
        assert!(log.iter().any(|line| line == "resolve site"), "{log:?}");
        assert!(log.iter().any(|line| line == "resolve account:ada"), "{log:?}");
        assert!(log.iter().any(|line| line == "page account:ada "), "{log:?}");
        assert!(log.iter().any(|line| line.starts_with("publish Hello")), "{log:?}");
    }
}
