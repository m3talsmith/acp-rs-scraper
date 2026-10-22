//! Capability traits.
//!
//! These are the traits an out-of-tree crate implements. They are not object-safe.
//! The host stores [`crate::DynService`]. A blanket impl turns a type that
//! implements [`Service`] and every capability trait into a [`crate::DynService`].
//! Default bodies return [`crate::ServiceError::NotImplemented`], so a service
//! overrides only the methods it provides.
//!
//! Do not import [`crate::DynService`] in the service crate. Its method names
//! match [`Service`]. With both traits in scope, a call such as `self.id()` is
//! ambiguous. The host imports [`crate::DynService`]. The service crate imports
//! [`Service`] and the capability traits.

use crate::abi::{CapabilityId, DynService};
use crate::records::*;

/// Identity, the capability list, the settings schema, and runtime availability.
pub trait Service: Send + Sync {
    /// Registrar id. This is the plan's `Service::id`.
    fn service_id(&self) -> ServiceId;
    /// Name shown in the Services screen.
    fn display_name(&self) -> &'static str;
    /// Traits this crate implements. Fixed for the life of the binary.
    fn implemented(&self) -> &'static [CapabilityId];
    /// Settings fields. `Config` goes to toml. `Secret` is sealed.
    fn schema(&self) -> &'static [Field];
    /// Logged-in, gated, over cap, and so on.
    fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability;

    /// This crate implements `cap`.
    fn has(&self, cap: CapabilityId) -> bool {
        self.implemented().contains(&cap)
    }

    /// The host calls this before the service is stored.
    ///
    /// The default does nothing. Store `host` in a [`crate::HostSlot`] when
    /// this service calls the core.
    fn attach(&mut self, _host: std::sync::Arc<dyn crate::host::Host>) {}

    /// [`schema`](Service::schema) plus the service id.
    fn config_schema(&self) -> ConfigSchema {
        ConfigSchema {
            service: self.service_id(),
            fields: self.schema(),
        }
    }
}

/// Session or API login. The returned bytes are sealed by the core.
pub trait Authenticator: Service {
    /// Exchange `fields` for secret material. The service does not write the cookie file.
    fn authenticate<'a>(
        &'a self,
        _route: RouteKind,
        _fields: &'a FilledFields,
    ) -> CapFut<'a, SealedMaterial> {
        not_implemented(
            self.service_id().as_str(),
            CapabilityId::Authenticator.as_str(),
        )
    }
}

/// Incremental, resumable read of accounts, searches, and threads.
pub trait Reader: Service {
    /// Turn `target` into a stable id.
    fn resolve<'a>(&'a self, _target: &'a Target) -> CapFut<'a, Resolved> {
        not_implemented(self.service_id().as_str(), CapabilityId::Reader.as_str())
    }

    /// One page, newest first. `stop_at` is an item id that ends an incremental sync.
    fn page<'a>(
        &'a self,
        _resolved: &'a Resolved,
        _cursor: &'a Cursor,
        _stop_at: Option<&'a str>,
        _budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        not_implemented(self.service_id().as_str(), CapabilityId::Reader.as_str())
    }

    /// More of one item: replies, the thread, media, a transcript, or comments.
    fn expand<'a>(
        &'a self,
        _item: &'a SourceId,
        _what: Expand,
        _budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        not_implemented(self.service_id().as_str(), CapabilityId::Reader.as_str())
    }
}

/// Updates for the open view, without a full scrape.
pub trait Live: Service {
    /// Incremental items for `view`.
    fn live<'a>(&'a self, _view: &'a SourceId) -> CapFut<'a, Vec<ItemRec>> {
        not_implemented(self.service_id().as_str(), CapabilityId::Live.as_str())
    }
}

/// A notice the core stores. The service does not own the notice table.
pub trait Notification: Service {
    /// Turn `event` into the notice the core keeps.
    fn notify<'a>(&'a self, _event: &'a NoticeRec) -> CapFut<'a, NoticeRec> {
        not_implemented(
            self.service_id().as_str(),
            CapabilityId::Notification.as_str(),
        )
    }
}

/// A new post.
pub trait Post: Service {
    /// Publish `draft`.
    fn publish<'a>(&'a self, _draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.service_id().as_str(), CapabilityId::Post.as_str())
    }
}

/// A reply.
pub trait Reply: Service {
    /// Publish `draft` as a reply. `draft.in_reply_to` is the parent.
    fn reply<'a>(&'a self, _draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.service_id().as_str(), CapabilityId::Reply.as_str())
    }
}

/// An attachment upload.
pub trait Upload: Service {
    /// Upload the attachments on `draft`.
    fn upload<'a>(&'a self, _draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.service_id().as_str(), CapabilityId::Upload.as_str())
    }
}

/// A vote or a like.
pub trait Vote: Service {
    /// Vote on `item`.
    fn vote<'a>(&'a self, _item: &'a SourceId, _choice: VoteChoice) -> CapFut<'a, ()> {
        not_implemented(self.service_id().as_str(), CapabilityId::Vote.as_str())
    }
}

/// A subscribe or a follow.
pub trait Subscribe: Service {
    /// Follow `account` when `enabled` is set, otherwise unfollow.
    fn subscribe<'a>(&'a self, _account: &'a SourceId, _enabled: bool) -> CapFut<'a, ()> {
        not_implemented(self.service_id().as_str(), CapabilityId::Subscribe.as_str())
    }
}

/// An edit of an existing item.
pub trait Edit: Service {
    /// Replace the body of `item`.
    fn edit<'a>(&'a self, _item: &'a SourceId, _body: &'a str) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.service_id().as_str(), CapabilityId::Edit.as_str())
    }
}

/// A delete.
pub trait Delete: Service {
    /// Delete `item`.
    fn delete<'a>(&'a self, _item: &'a SourceId) -> CapFut<'a, ()> {
        not_implemented(self.service_id().as_str(), CapabilityId::Delete.as_str())
    }
}

/// A repost or a quote.
pub trait Repost: Service {
    /// Repost `item`. `quote` is the quote text. A service may refuse an empty quote.
    fn repost<'a>(
        &'a self,
        _item: &'a SourceId,
        _quote: Option<&'a str>,
    ) -> CapFut<'a, WriteOutcome> {
        not_implemented(self.service_id().as_str(), CapabilityId::Repost.as_str())
    }
}

/// Add an item to a research project. The core store is the local implementation.
pub trait Research: Service {
    /// Record `item` against a project.
    fn add_research<'a>(&'a self, _item: &'a ResearchRef) -> CapFut<'a, ()> {
        not_implemented(self.service_id().as_str(), CapabilityId::Research.as_str())
    }
}

/// Refresh a feed rule.
pub trait Feeds: Service {
    /// Rows for `rule`. The core inserts them.
    fn refresh<'a>(&'a self, _rule: &'a FeedRule, _budget: &'a mut Budget) -> CapFut<'a, Page> {
        not_implemented(self.service_id().as_str(), CapabilityId::Feeds.as_str())
    }
}

/// Video, audio, transcripts, thumbnails, and attachments.
pub trait MediaFetcher: Service {
    /// Bytes for a URL the service already resolved.
    fn fetch<'a>(
        &'a self,
        _req: &'a MediaRequest,
        _budget: &'a mut Budget,
    ) -> CapFut<'a, MediaBytes> {
        not_implemented(
            self.service_id().as_str(),
            CapabilityId::MediaFetcher.as_str(),
        )
    }
}

/// Red Star Spaces. This is not an X Spaces read.
pub trait Spaces: Service {
    /// One page of Spaces for `target`.
    fn spaces<'a>(
        &'a self,
        _target: &'a Target,
        _cursor: &'a Cursor,
        _budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        not_implemented(self.service_id().as_str(), CapabilityId::Spaces.as_str())
    }
}

/// A post stored on this machine before it is sent.
pub trait LocalPost: Service {
    /// Save `draft` under `id`. `keep` true leaves it off the send queue.
    fn save_local<'a>(&'a self, _id: &'a str, _draft: &'a Draft, _keep: bool) -> CapFut<'a, String> {
        not_implemented(self.service_id().as_str(), CapabilityId::LocalPost.as_str())
    }

    /// Drafts and keep-local posts, newest update first.
    fn list_local<'a>(&'a self) -> CapFut<'a, Vec<String>> {
        not_implemented(self.service_id().as_str(), CapabilityId::LocalPost.as_str())
    }
}

/// Local drafts and the send queue.
pub trait Outbox: Service {
    /// Queue a saved local post.
    fn enqueue<'a>(&'a self, _local_id: &'a str) -> CapFut<'a, String> {
        not_implemented(self.service_id().as_str(), CapabilityId::Outbox.as_str())
    }

    /// Queue rows, newest first. An empty queue is one line, `outbox none`.
    fn queued<'a>(&'a self) -> CapFut<'a, Vec<String>> {
        not_implemented(self.service_id().as_str(), CapabilityId::Outbox.as_str())
    }

    /// Cancel a queued item. Returns the host's status line.
    fn cancel_queued<'a>(&'a self, _id: &'a str) -> CapFut<'a, String> {
        not_implemented(self.service_id().as_str(), CapabilityId::Outbox.as_str())
    }
}

/// Estimate and ledger. Required on a paid or quota API route.
pub trait CostMeter: Service {
    /// The estimate for what `budget` still allows.
    fn estimate<'a>(&'a self, _budget: &'a Budget) -> CapFut<'a, Spend> {
        not_implemented(self.service_id().as_str(), CapabilityId::CostMeter.as_str())
    }
}

/// Replace the signed-in account's profile. The host stores the returned row.
pub trait UpdateProfile: Service {
    /// Write `account`. The returned row is what the host stores.
    fn update_profile<'a>(&'a self, _account: &'a AccountRec) -> CapFut<'a, AccountRec> {
        not_implemented(
            self.service_id().as_str(),
            CapabilityId::UpdateProfile.as_str(),
        )
    }
}

/// Report one post. The host confirms before it calls this.
pub trait ReportPost: Service {
    /// Report `item` for `reason`.
    fn report_post<'a>(&'a self, _item: &'a SourceId, _reason: &'a str) -> CapFut<'a, ()> {
        not_implemented(self.service_id().as_str(), CapabilityId::ReportPost.as_str())
    }
}

/// Report one account. The host confirms before it calls this.
pub trait ReportUser: Service {
    /// Report `account` for `reason`.
    fn report_user<'a>(&'a self, _account: &'a SourceId, _reason: &'a str) -> CapFut<'a, ()> {
        not_implemented(self.service_id().as_str(), CapabilityId::ReportUser.as_str())
    }
}

/// Bookmark or unbookmark one post.
///
/// The host records the mark with [`crate::Host::bookmark_post`]. This trait is
/// the site half. `on` false clears the bookmark.
pub trait Bookmark: Service {
    /// Bookmark `item` when `on` is set.
    fn bookmark<'a>(&'a self, _item: &'a SourceId, _on: bool) -> CapFut<'a, ()> {
        not_implemented(self.service_id().as_str(), CapabilityId::Bookmark.as_str())
    }
}

impl<T> DynService for T
where
    T: Service
        + Authenticator
        + Reader
        + Live
        + Notification
        + Post
        + Reply
        + Upload
        + Vote
        + Subscribe
        + Edit
        + Delete
        + Repost
        + Research
        + Feeds
        + MediaFetcher
        + Spaces
        + CostMeter
        + LocalPost
        + Outbox
        + UpdateProfile
        + ReportPost
        + ReportUser
        + Bookmark,
{
    fn id(&self) -> &'static str {
        self.service_id().as_str()
    }

    fn display_name(&self) -> &'static str {
        Service::display_name(self)
    }

    fn implemented(&self) -> &'static [CapabilityId] {
        Service::implemented(self)
    }

    fn schema(&self) -> &'static [Field] {
        Service::schema(self)
    }

    fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
        Service::availability(self, cap, ctx)
    }

    fn attach(&mut self, host: std::sync::Arc<dyn crate::host::Host>) {
        Service::attach(self, host);
    }

    fn authenticate<'a>(
        &'a self,
        route: RouteKind,
        fields: &'a FilledFields,
    ) -> CapFut<'a, SealedMaterial> {
        Authenticator::authenticate(self, route, fields)
    }

    fn resolve<'a>(&'a self, target: &'a Target) -> CapFut<'a, Resolved> {
        Reader::resolve(self, target)
    }

    fn page<'a>(
        &'a self,
        resolved: &'a Resolved,
        cursor: &'a Cursor,
        stop_at: Option<&'a str>,
        budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        Reader::page(self, resolved, cursor, stop_at, budget)
    }

    fn expand<'a>(
        &'a self,
        item: &'a SourceId,
        what: Expand,
        budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        Reader::expand(self, item, what, budget)
    }

    fn live<'a>(&'a self, view: &'a SourceId) -> CapFut<'a, Vec<ItemRec>> {
        Live::live(self, view)
    }

    fn notify<'a>(&'a self, event: &'a NoticeRec) -> CapFut<'a, NoticeRec> {
        Notification::notify(self, event)
    }

    fn publish<'a>(&'a self, draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        Post::publish(self, draft)
    }

    fn reply<'a>(&'a self, draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        Reply::reply(self, draft)
    }

    fn upload<'a>(&'a self, draft: &'a Draft) -> CapFut<'a, WriteOutcome> {
        Upload::upload(self, draft)
    }

    fn vote<'a>(&'a self, item: &'a SourceId, choice: VoteChoice) -> CapFut<'a, ()> {
        Vote::vote(self, item, choice)
    }

    fn subscribe<'a>(&'a self, account: &'a SourceId, enabled: bool) -> CapFut<'a, ()> {
        Subscribe::subscribe(self, account, enabled)
    }

    fn edit<'a>(&'a self, item: &'a SourceId, body: &'a str) -> CapFut<'a, WriteOutcome> {
        Edit::edit(self, item, body)
    }

    fn delete<'a>(&'a self, item: &'a SourceId) -> CapFut<'a, ()> {
        Delete::delete(self, item)
    }

    fn repost<'a>(
        &'a self,
        item: &'a SourceId,
        quote: Option<&'a str>,
    ) -> CapFut<'a, WriteOutcome> {
        Repost::repost(self, item, quote)
    }

    fn add_research<'a>(&'a self, item: &'a ResearchRef) -> CapFut<'a, ()> {
        Research::add_research(self, item)
    }

    fn refresh<'a>(&'a self, rule: &'a FeedRule, budget: &'a mut Budget) -> CapFut<'a, Page> {
        Feeds::refresh(self, rule, budget)
    }

    fn fetch<'a>(
        &'a self,
        req: &'a MediaRequest,
        budget: &'a mut Budget,
    ) -> CapFut<'a, MediaBytes> {
        MediaFetcher::fetch(self, req, budget)
    }

    fn estimate<'a>(&'a self, budget: &'a Budget) -> CapFut<'a, Spend> {
        CostMeter::estimate(self, budget)
    }

    fn spaces<'a>(
        &'a self,
        target: &'a Target,
        cursor: &'a Cursor,
        budget: &'a mut Budget,
    ) -> CapFut<'a, Page> {
        Spaces::spaces(self, target, cursor, budget)
    }

    fn save_local<'a>(&'a self, id: &'a str, draft: &'a Draft, keep: bool) -> CapFut<'a, String> {
        LocalPost::save_local(self, id, draft, keep)
    }

    fn list_local<'a>(&'a self) -> CapFut<'a, Vec<String>> {
        LocalPost::list_local(self)
    }

    fn enqueue<'a>(&'a self, local_id: &'a str) -> CapFut<'a, String> {
        Outbox::enqueue(self, local_id)
    }

    fn queued<'a>(&'a self) -> CapFut<'a, Vec<String>> {
        Outbox::queued(self)
    }

    fn cancel_queued<'a>(&'a self, id: &'a str) -> CapFut<'a, String> {
        Outbox::cancel_queued(self, id)
    }

    fn update_profile<'a>(&'a self, account: &'a AccountRec) -> CapFut<'a, AccountRec> {
        UpdateProfile::update_profile(self, account)
    }

    fn report_post<'a>(&'a self, item: &'a SourceId, reason: &'a str) -> CapFut<'a, ()> {
        ReportPost::report_post(self, item, reason)
    }

    fn report_user<'a>(&'a self, account: &'a SourceId, reason: &'a str) -> CapFut<'a, ()> {
        ReportUser::report_user(self, account, reason)
    }

    fn bookmark<'a>(&'a self, item: &'a SourceId, on: bool) -> CapFut<'a, ()> {
        Bookmark::bookmark(self, item, on)
    }
}

/// Implement every capability trait with its default body, except the ones named.
///
/// A service writes its own `impl Reader for Ty` (and any other trait it
/// overrides), then calls this macro for the rest. `except` takes a
/// comma-separated list: `impl_default_caps!(Ty, except Reader, Post)`.
/// Together with [`Service`], that satisfies the blanket
/// [`DynService`](crate::DynService) impl.
#[macro_export]
macro_rules! impl_default_caps {
    ($ty:ty) => { $crate::impl_default_caps!($ty, except); };
    ($ty:ty, except $($skip:ident),* $(,)?) => {
        $crate::__impl_one_cap!(Authenticator, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Reader, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Live, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Notification, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Post, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Reply, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Upload, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Vote, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Subscribe, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Edit, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Delete, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Repost, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Research, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Feeds, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(MediaFetcher, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Spaces, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(CostMeter, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(LocalPost, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Outbox, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(UpdateProfile, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(ReportPost, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(ReportUser, $ty, [$($skip),*]);
        $crate::__impl_one_cap!(Bookmark, $ty, [$($skip),*]);
    };
}

/// Helper for [`impl_default_caps!`]. Not a stable entry point.
#[macro_export]
#[doc(hidden)]
macro_rules! __impl_one_cap {
    (Authenticator, $ty:ty, [Authenticator $(, $rest:ident)*]) => {};
    (Authenticator, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Authenticator, $ty, [$($rest),*]);
    };
    (Authenticator, $ty:ty, []) => { impl $crate::Authenticator for $ty {} };

    (Reader, $ty:ty, [Reader $(, $rest:ident)*]) => {};
    (Reader, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Reader, $ty, [$($rest),*]);
    };
    (Reader, $ty:ty, []) => { impl $crate::Reader for $ty {} };

    (Live, $ty:ty, [Live $(, $rest:ident)*]) => {};
    (Live, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Live, $ty, [$($rest),*]);
    };
    (Live, $ty:ty, []) => { impl $crate::Live for $ty {} };

    (Notification, $ty:ty, [Notification $(, $rest:ident)*]) => {};
    (Notification, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Notification, $ty, [$($rest),*]);
    };
    (Notification, $ty:ty, []) => { impl $crate::Notification for $ty {} };

    (Post, $ty:ty, [Post $(, $rest:ident)*]) => {};
    (Post, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Post, $ty, [$($rest),*]);
    };
    (Post, $ty:ty, []) => { impl $crate::Post for $ty {} };

    (Reply, $ty:ty, [Reply $(, $rest:ident)*]) => {};
    (Reply, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Reply, $ty, [$($rest),*]);
    };
    (Reply, $ty:ty, []) => { impl $crate::Reply for $ty {} };

    (Upload, $ty:ty, [Upload $(, $rest:ident)*]) => {};
    (Upload, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Upload, $ty, [$($rest),*]);
    };
    (Upload, $ty:ty, []) => { impl $crate::Upload for $ty {} };

    (Vote, $ty:ty, [Vote $(, $rest:ident)*]) => {};
    (Vote, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Vote, $ty, [$($rest),*]);
    };
    (Vote, $ty:ty, []) => { impl $crate::Vote for $ty {} };

    (Subscribe, $ty:ty, [Subscribe $(, $rest:ident)*]) => {};
    (Subscribe, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Subscribe, $ty, [$($rest),*]);
    };
    (Subscribe, $ty:ty, []) => { impl $crate::Subscribe for $ty {} };

    (Edit, $ty:ty, [Edit $(, $rest:ident)*]) => {};
    (Edit, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Edit, $ty, [$($rest),*]);
    };
    (Edit, $ty:ty, []) => { impl $crate::Edit for $ty {} };

    (Delete, $ty:ty, [Delete $(, $rest:ident)*]) => {};
    (Delete, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Delete, $ty, [$($rest),*]);
    };
    (Delete, $ty:ty, []) => { impl $crate::Delete for $ty {} };

    (Repost, $ty:ty, [Repost $(, $rest:ident)*]) => {};
    (Repost, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Repost, $ty, [$($rest),*]);
    };
    (Repost, $ty:ty, []) => { impl $crate::Repost for $ty {} };

    (Research, $ty:ty, [Research $(, $rest:ident)*]) => {};
    (Research, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Research, $ty, [$($rest),*]);
    };
    (Research, $ty:ty, []) => { impl $crate::Research for $ty {} };

    (Feeds, $ty:ty, [Feeds $(, $rest:ident)*]) => {};
    (Feeds, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Feeds, $ty, [$($rest),*]);
    };
    (Feeds, $ty:ty, []) => { impl $crate::Feeds for $ty {} };

    (MediaFetcher, $ty:ty, [MediaFetcher $(, $rest:ident)*]) => {};
    (MediaFetcher, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(MediaFetcher, $ty, [$($rest),*]);
    };
    (MediaFetcher, $ty:ty, []) => { impl $crate::MediaFetcher for $ty {} };

    (Spaces, $ty:ty, [Spaces $(, $rest:ident)*]) => {};
    (Spaces, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Spaces, $ty, [$($rest),*]);
    };
    (Spaces, $ty:ty, []) => { impl $crate::Spaces for $ty {} };

    (CostMeter, $ty:ty, [CostMeter $(, $rest:ident)*]) => {};
    (CostMeter, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(CostMeter, $ty, [$($rest),*]);
    };
    (CostMeter, $ty:ty, []) => { impl $crate::CostMeter for $ty {} };

    (LocalPost, $ty:ty, [LocalPost $(, $rest:ident)*]) => {};
    (LocalPost, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(LocalPost, $ty, [$($rest),*]);
    };
    (LocalPost, $ty:ty, []) => { impl $crate::LocalPost for $ty {} };

    (Outbox, $ty:ty, [Outbox $(, $rest:ident)*]) => {};
    (Outbox, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Outbox, $ty, [$($rest),*]);
    };
    (Outbox, $ty:ty, []) => { impl $crate::Outbox for $ty {} };

    (UpdateProfile, $ty:ty, [UpdateProfile $(, $rest:ident)*]) => {};
    (UpdateProfile, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(UpdateProfile, $ty, [$($rest),*]);
    };
    (UpdateProfile, $ty:ty, []) => { impl $crate::UpdateProfile for $ty {} };

    (ReportPost, $ty:ty, [ReportPost $(, $rest:ident)*]) => {};
    (ReportPost, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(ReportPost, $ty, [$($rest),*]);
    };
    (ReportPost, $ty:ty, []) => { impl $crate::ReportPost for $ty {} };

    (ReportUser, $ty:ty, [ReportUser $(, $rest:ident)*]) => {};
    (ReportUser, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(ReportUser, $ty, [$($rest),*]);
    };
    (ReportUser, $ty:ty, []) => { impl $crate::ReportUser for $ty {} };

    (Bookmark, $ty:ty, [Bookmark $(, $rest:ident)*]) => {};
    (Bookmark, $ty:ty, [$other:ident $(, $rest:ident)*]) => {
        $crate::__impl_one_cap!(Bookmark, $ty, [$($rest),*]);
    };
    (Bookmark, $ty:ty, []) => { impl $crate::Bookmark for $ty {} };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{CORE_VERSION, PluginRegistrar, RUSTC_VERSION};
    use crate::declare_plugin;
    use std::future::Future;

    #[derive(Default)]
    struct Demo;

    impl Service for Demo {
        fn service_id(&self) -> ServiceId {
            ServiceId::new("demo")
        }
        fn display_name(&self) -> &'static str {
            "Demo"
        }
        fn implemented(&self) -> &'static [CapabilityId] {
            &[CapabilityId::Reader]
        }
        fn schema(&self) -> &'static [Field] {
            &[]
        }
        fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
            if ctx.enabled && Service::has(self, cap) {
                Availability::Ready
            } else {
                Availability::Off
            }
        }
    }

    impl Reader for Demo {
        fn page<'a>(
            &'a self,
            _resolved: &'a Resolved,
            _cursor: &'a Cursor,
            _stop_at: Option<&'a str>,
            budget: &'a mut Budget,
        ) -> CapFut<'a, Page> {
            let charged = budget.try_spend(Spend::new(1, 0));
            Box::pin(async move {
                charged?;
                let mut page = Page::empty();
                page.cost = Spend::new(1, 0);
                page.items.push(ItemRec::new(
                    SourceId::new(ServiceId::new("demo"), "1"),
                    "hello",
                ));
                Ok(page)
            })
        }
    }

    impl_default_caps!(Demo, except Reader);
    declare_plugin!(Demo);

    #[test]
    fn a_git_rev_service_registers_and_pages() {
        assert_eq!(crate::SESSION_SCHEMA[0].key, "email");
        assert_eq!(crate::SESSION_SCHEMA[1].key, "base_url");
        assert!(crate::SESSION_SCHEMA.iter().all(|field| field.store == Store::Config));
        assert_eq!(plugin_declaration.core_version, CORE_VERSION);
        assert_eq!(plugin_declaration.rustc_version, RUSTC_VERSION);
        let mut reg = Rec::default();
        unsafe { (plugin_declaration.register)(&mut reg) };
        assert_eq!(reg.ids, vec!["demo".to_string()]);

        let demo = Demo;
        let sid = SourceId::new(ServiceId::new("demo"), "acct");
        let resolved = Resolved {
            target: Target::Account {
                handle: "ada".into(),
            },
            source_id: sid,
        };
        let mut budget = Budget::capped(1, 0);
        let page = block(DynService::page(
            &demo,
            &resolved,
            &Cursor::default(),
            None,
            &mut budget,
        ))
        .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].body, "hello");
        assert_eq!(budget.remaining_items, Some(0));
        let err = block(DynService::vote(&demo, &resolved.source_id, VoteChoice::Up)).unwrap_err();
        assert!(err.to_string().contains("does not implement vote"), "{err}");
        let err = block(DynService::spaces(
            &demo,
            &resolved.target,
            &Cursor::default(),
            &mut budget,
        ))
        .unwrap_err();
        assert!(err.to_string().contains("does not implement spaces"), "{err}");
    }

    #[test]
    fn a_refused_charge_leaves_the_budget() {
        let mut budget = Budget::capped(1, 5);
        assert!(budget.try_spend(Spend::new(2, 0)).is_err());
        assert_eq!(budget.remaining_items, Some(1));
        assert_eq!(budget.remaining_micros, Some(5));
        assert!(budget.try_spend(Spend::new(1, 5)).is_ok());
        assert_eq!(budget.remaining_items, Some(0));
        assert_eq!(budget.remaining_micros, Some(0));
    }

    #[derive(Default)]
    struct Two;

    impl Service for Two {
        fn service_id(&self) -> ServiceId {
            ServiceId::new("two")
        }
        fn display_name(&self) -> &'static str {
            "Two"
        }
        fn implemented(&self) -> &'static [CapabilityId] {
            &[CapabilityId::Reader, CapabilityId::Post]
        }
        fn schema(&self) -> &'static [Field] {
            &[]
        }
        fn availability(&self, cap: CapabilityId, ctx: &RuntimeCtx) -> Availability {
            if ctx.enabled && Service::has(self, cap) {
                Availability::Ready
            } else {
                Availability::Off
            }
        }
    }

    impl Reader for Two {}
    impl Post for Two {}
    impl_default_caps!(Two, except Reader, Post);

    #[test]
    fn except_takes_a_comma_separated_list() {
        let two = Two;
        let sid = SourceId::new(ServiceId::new("two"), "1");
        let err = block(DynService::vote(&two, &sid, VoteChoice::Up)).unwrap_err();
        assert!(err.to_string().contains("does not implement vote"), "{err}");
        let err = block(DynService::enqueue(&two, "local-1")).unwrap_err();
        assert!(err.to_string().contains("does not implement outbox"), "{err}");
        let err = block(DynService::list_local(&two)).unwrap_err();
        assert!(err.to_string().contains("does not implement local_post"), "{err}");
    }

    #[derive(Default)]
    struct Rec {
        ids: Vec<String>,
    }

    impl PluginRegistrar for Rec {
        fn register_service(&mut self, service: Box<dyn DynService>) {
            self.ids.push(service.id().to_string());
        }
    }

    fn block<F: Future>(f: F) -> F::Output {
        let mut f = Box::pin(f);
        let waker = std::task::Waker::from(std::sync::Arc::new(Nop));
        let mut cx = std::task::Context::from_waker(&waker);
        match f.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => panic!("the future should be ready"),
        }
    }

    struct Nop;
    impl std::task::Wake for Nop {
        fn wake(self: std::sync::Arc<Self>) {}
    }
}
