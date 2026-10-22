//! Reader plugin for the loader tests. It does not talk to a site.

#![warn(missing_docs)]

use acp_rs_plugin::{
    Availability, Budget, CapabilityId, Cursor, DynService, ItemRec, Page, Resolved, RuntimeCtx,
    ServiceId, SourceId, Target,
};

const BODY: &str = "from the fixture";

/// Reader the host loads from a verified archive.
#[derive(Debug, Default)]
pub struct Fixture;

impl DynService for Fixture {
    fn id(&self) -> &'static str {
        "fixture"
    }

    fn display_name(&self) -> &'static str {
        "Fixture"
    }

    fn implemented(&self) -> &'static [CapabilityId] {
        &[CapabilityId::Reader]
    }

    fn schema(&self) -> &'static [acp_rs_plugin::Field] {
        &[]
    }

    fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
        if ctx.enabled && self.has(cap) {
            Availability::Ready
        } else {
            Availability::Off
        }
    }

    fn resolve<'a>(&'a self, target: &'a Target) -> acp_rs_plugin::CapFut<'a, Resolved> {
        let native = match target {
            Target::Account { handle } => format!("account:{handle}"),
            Target::Bookmarks => "bookmarks".into(),
            Target::Site => "site".into(),
            Target::Search { query } => format!("search:{query}"),
            Target::Playlist { id } => format!("list:{id}"),
            Target::Item { id } | Target::Conversation { id } => id.native.clone(),
        };
        let source_id = SourceId::new(ServiceId::new("fixture"), native);
        let target = target.clone();
        Box::pin(async move { Ok(Resolved { target, source_id }) })
    }

    fn page<'a>(
        &'a self,
        _resolved: &'a Resolved,
        _cursor: &'a Cursor,
        _stop_at: Option<&'a str>,
        _budget: &'a mut Budget,
    ) -> acp_rs_plugin::CapFut<'a, Page> {
        let mut item = ItemRec::new(SourceId::new(ServiceId::new("fixture"), "fixture-1"), BODY);
        item.nick = "ada".into();
        let mut page = Page::empty();
        page.items.push(item);
        Box::pin(async move { Ok(page) })
    }
}

acp_rs_plugin::declare_plugin!(Fixture);
