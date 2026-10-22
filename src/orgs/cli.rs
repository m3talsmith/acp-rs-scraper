//! `--org-*` flags. Writes stay on this machine. Making something public asks first.

use crate::orgs::{Audience, Org, OrgEdit, OrgPost, Visibility, add_post, create_org, delete_org, get_org, list_orgs, list_posts, org_conn, post_count, stored_posts, update_org};
use crate::prelude::*;

/// True when an organization flag was set.
pub fn cli_requested(args: &Args) -> bool {
    args.org_list
        || args.org_create.is_some()
        || args.org_edit.is_some()
        || args.org_delete.is_some()
        || args.org_show.is_some()
        || args.org_roles.is_some()
        || args.org_members.is_some()
        || args.org_snapshot.is_some()
        || args.org_template_show.is_some()
        || args.org_template_set.is_some()
        || args.org_template_reset.is_some()
        || args.org_template_file.is_some()
        || args.org_asset_add.is_some()
        || args.org_asset_list.is_some()
        || !args.org_asset_visibility.is_empty()
        || args.org_asset_remove.is_some()
        || args.org_post.is_some()
        || args.org_name.is_some()
        || args.org_description.is_some()
        || args.org_visibility.is_some()
        || args.org_default_visibility.is_some()
        || args.org_title.is_some()
        || args.org_post_visibility.is_some()
}

/// Run the organization flag that was set.
pub fn run_cli(args: &Args) -> Result<()> {
    let db = home()?.join(DB_REL_PATH);
    let json_out = std::env::args().any(|a| a == "--format" || a.starts_with("--format=")) && args.formats().contains(&Format::Json);
    if args.org_list {
        let c = org_conn(&db)?;
        let orgs = list_orgs(&c)?;
        print_list(&c, &orgs, json_out)?;
        return Ok(());
    }
    if let Some(slug) = &args.org_create {
        return create(args, &db, slug);
    }
    if let Some(slug) = &args.org_edit {
        return edit(args, &db, slug);
    }
    if let Some(slug) = &args.org_delete {
        return remove(args, &db, slug);
    }
    if let Some(slug) = &args.org_show {
        let c = org_conn(&db)?;
        let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?} (--org-list shows them)"))?;
        let posts = list_posts(&c, org.id)?;
        print_show(&org, &posts, json_out)?;
        return Ok(());
    }
    if let Some(slug) = &args.org_roles {
        let c = org_conn(&db)?;
        let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
        for role in crate::orgs::access::roles(&c, org.id)? {
            println!("{}  rank {}  {}", role.name, role.rank, role.permissions);
        }
        return Ok(());
    }
    if let Some(slug) = &args.org_members {
        let c = org_conn(&db)?;
        let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
        for member in crate::orgs::access::members(&c, org.id)? {
            println!("{}  {}  {}", member.identity_pub, member.role, member.status.as_str());
        }
        return Ok(());
    }
    if let Some(slug) = &args.org_snapshot {
        let c = org_conn(&db)?;
        let doc = crate::orgs::public_snapshot(&c, slug)?;
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(());
    }
    if let Some(slug) = &args.org_template_show {
        return show_template(&org_conn(&db)?, slug);
    }
    if let Some(slug) = &args.org_template_set {
        return set_template(args, &db, slug);
    }
    if let Some(slug) = &args.org_template_reset {
        return reset_template(args, &db, slug);
    }
    if let Some(slug) = &args.org_asset_add {
        return add_asset(args, &db, slug);
    }
    if let Some(slug) = &args.org_asset_list {
        return list_assets(&org_conn(&db)?, slug);
    }
    if args.org_asset_visibility.len() == 2 {
        return asset_visibility(args, &db);
    }
    if let Some(id) = &args.org_asset_remove {
        return remove_asset(args, &db, id);
    }
    if let Some(slug) = &args.org_post {
        return post(args, &db, slug);
    }
    bail!("--org-name, --org-description, --org-visibility, --org-default-visibility, --org-title, and --org-post-visibility belong to --org-create, --org-edit, or --org-post");
}

fn create(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let name = args.org_name.clone().unwrap_or_else(|| slug.trim().to_string());
    let description = args.org_description.clone().unwrap_or_default();
    let visibility = flag_vis(args.org_visibility.as_deref(), Visibility::Private)?;
    let default_post_visibility = flag_vis(args.org_default_visibility.as_deref(), Visibility::Private)?;
    let preview = format!(
        "would create organization {slug} ({name}), visibility {}, default post visibility {}",
        visibility.as_str(),
        default_post_visibility.as_str()
    );
    if args.dry_run {
        println!("{preview}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if visibility == Visibility::Public || default_post_visibility == Visibility::Public {
        println!("{preview}");
        if !confirm_public(args)? {
            return Ok(());
        }
    }
    let owner = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let org = create_org(&c, slug, &name, &description, visibility, default_post_visibility, &owner)?;
    info!("organization {} created", org.slug);
    println!("{}  visibility: {}  default post visibility: {}", org.slug, org.visibility.as_str(), org.default_post_visibility.as_str());
    println!("gated: {}", if org.gated { "yes" } else { "no" });
    println!("registration: {}", org.registration.as_str());
    Ok(())
}

fn edit(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let visibility = args.org_visibility.as_deref().map(Visibility::parse).transpose()?;
    let default_post_visibility = args.org_default_visibility.as_deref().map(Visibility::parse).transpose()?;
    if args.org_name.is_none() && args.org_description.is_none() && visibility.is_none() && default_post_visibility.is_none() {
        bail!("--org-edit needs --org-name, --org-description, --org-visibility, or --org-default-visibility");
    }
    let exposing = visibility == Some(Visibility::Public) && org.visibility != Visibility::Public
        || default_post_visibility == Some(Visibility::Public) && org.default_post_visibility != Visibility::Public;
    let preview = format!(
        "would edit organization {}: name {:?}, visibility {}, default post visibility {}",
        org.slug,
        args.org_name.as_deref().unwrap_or(&org.name),
        visibility.unwrap_or(org.visibility).as_str(),
        default_post_visibility.unwrap_or(org.default_post_visibility).as_str()
    );
    if args.dry_run {
        println!("{preview}");
        if exposing {
            println!("would make public content readable. dry-run: nothing was written");
        } else {
            println!("dry-run: nothing was written");
        }
        return Ok(());
    }
    if exposing && !confirm_public(args)? {
        return Ok(());
    }
    let updated = update_org(
        &c,
        &org.slug,
        &OrgEdit { name: args.org_name.clone(), description: args.org_description.clone(), visibility, default_post_visibility },
    )?;
    info!("organization {:?} updated", updated.slug);
    println!("{}  visibility: {}  default post visibility: {}", updated.slug, updated.visibility.as_str(), updated.default_post_visibility.as_str());
    Ok(())
}

fn remove(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let n = post_count(&c, org.id)?;
    println!("would delete organization {} and its {n} posts", org.slug);
    if args.dry_run {
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Delete organization? [y/N] ")? {
        println!("cancelled");
        return Ok(());
    }
    delete_org(&c, &org.slug)?;
    info!("organization {:?} deleted", org.slug);
    println!("deleted {}", org.slug);
    Ok(())
}

fn post(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let title = args.org_title.clone().unwrap_or_default();
    if title.trim().is_empty() {
        bail!("--org-post needs --org-title");
    }
    let body = post_body(args)?;
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let visibility = args.org_post_visibility.as_deref().map(Visibility::parse).transpose()?;
    let effective = visibility.unwrap_or(org.default_post_visibility);
    let preview = format!(
        "would post {title:?} in {} as the author, post visibility {}",
        org.slug,
        effective.as_str()
    );
    if args.dry_run {
        println!("{preview}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if readable_public(org.visibility, effective) && !confirm_public(args)? {
        println!("{preview}");
        return Ok(());
    }
    let (_, post) = add_post(&c, &org.slug, &title, &body, visibility)?;
    info!("organization {:?} post {} stored", org.slug, post.id);
    println!("{}  {}  post visibility: {}", org.slug, post.id, post.visibility.as_str());
    Ok(())
}

fn show_template(c: &Connection, slug: &str) -> Result<()> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    println!("{}", crate::orgs::template::source(c, org.id)?);
    Ok(())
}

fn set_template(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let path = args.org_template_file.as_ref().ok_or_else(|| anyhow!("--org-template-set needs --org-template-file"))?;
    let html = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let preview = format!("would replace the template for {slug} from {}", path.display());
    if args.dry_run {
        println!("{preview}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Replace this template? [y/N] ")? {
        println!("{preview}");
        println!("cancelled");
        return Ok(());
    }
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    crate::orgs::template::set_source(&c, org.id, &html)?;
    println!("template for {slug} replaced");
    Ok(())
}

fn reset_template(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let preview = format!("would restore the default template for {slug}");
    if args.dry_run {
        println!("{preview}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Restore the default template? [y/N] ")? {
        println!("{preview}");
        println!("cancelled");
        return Ok(());
    }
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    crate::orgs::template::reset_source(&c, org.id)?;
    println!("template for {slug} restored");
    Ok(())
}

fn add_asset(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let path = args.file.as_ref().ok_or_else(|| anyhow!("--org-asset-add needs --file"))?;
    let preview = format!("would copy {} into {slug} as a private file", path.display());
    if args.dry_run {
        println!("{preview}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Add this private file? [y/N] ")? {
        println!("{preview}");
        println!("cancelled");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let asset = crate::orgs::assets::add_file(&c, db, slug, &actor, path)?;
    println!("{}  {}  visibility: {}", asset.id, asset.name, asset.visibility.as_str());
    Ok(())
}

fn list_assets(c: &Connection, slug: &str) -> Result<()> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let rows = crate::orgs::access::list_assets(c, org.id)?;
    if rows.is_empty() {
        println!("No files yet (--org-asset-add {slug} --file PATH).");
        return Ok(());
    }
    for asset in rows {
        let archived = if asset.archived { "  archived" } else { "" };
        println!("{}  {}  visibility: {}{archived}", asset.id, asset.name, asset.visibility.as_str());
    }
    Ok(())
}

fn asset_visibility(args: &Args, db: &Path) -> Result<()> {
    let id = &args.org_asset_visibility[0];
    let visibility = Visibility::parse(&args.org_asset_visibility[1])?;
    let preview = format!("would set {id} to {}", visibility.as_str());
    if args.dry_run {
        println!("{preview}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if visibility == Visibility::Public && !confirm_public(args)? {
        println!("{preview}");
        println!("cancelled");
        return Ok(());
    }
    if visibility == Visibility::Private && !confirm(args, "Make this file private? [y/N] ")? {
        println!("{preview}");
        println!("cancelled");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let asset = crate::orgs::assets::set_visibility(&c, &actor, id, visibility)?;
    println!("{}  {}  visibility: {}", asset.id, asset.name, asset.visibility.as_str());
    Ok(())
}

fn remove_asset(args: &Args, db: &Path, id: &str) -> Result<()> {
    let preview = format!("would archive {id}. The file stays on disk. This is not a delete.");
    if args.dry_run {
        println!("{preview}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Archive this file? [y/N] ")? {
        println!("{preview}");
        println!("cancelled");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let record = crate::orgs::assets::remove(&c, &actor, id)?;
    println!("archived {id}");
    println!("replication: {}", record.kind);
    Ok(())
}

fn readable_public(org: Visibility, post: Visibility) -> bool {
    crate::orgs::readable_by_others(org, post)
}

fn flag_vis(raw: Option<&str>, default: Visibility) -> Result<Visibility> {
    match raw {
        Some(s) => Visibility::parse(s),
        None => Ok(default),
    }
}

fn post_body(args: &Args) -> Result<String> {
    if let Some(path) = &args.body_file {
        return std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()));
    }
    Ok(args.body.clone().unwrap_or_default())
}

fn confirm_public(args: &Args) -> Result<bool> {
    println!("This would make content readable by others. Cancel is the default.");
    confirm(args, "Make this public? [y/N] ")
}

fn confirm(args: &Args, prompt: &str) -> Result<bool> {
    if args.yes {
        return Ok(true);
    }
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!("refused: stdin is not a terminal (pass --yes). Cancel is the default.");
    }
    let line = crate::cli::prompt(prompt)?;
    Ok(line.eq_ignore_ascii_case("y") || line.eq_ignore_ascii_case("yes"))
}

fn print_list(c: &Connection, orgs: &[Org], json_out: bool) -> Result<()> {
    if json_out {
        let v: Vec<Value> = orgs
            .iter()
            .map(|o| {
                json!({
                    "slug": o.slug,
                    "name": o.name,
                    "description": o.description,
                    "visibility": o.visibility.as_str(),
                    "default_post_visibility": o.default_post_visibility.as_str(),
                    "posts": post_count(c, o.id).unwrap_or(0),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    if orgs.is_empty() {
        println!("No organizations yet (--org-create SLUG).");
        return Ok(());
    }
    println!("{:<16} {:<10} {:<10} {:>5}  NAME", "SLUG", "VISIBILITY", "DEFAULT", "POSTS");
    for o in orgs {
        println!(
            "{:<16} {:<10} {:<10} {:>5}  {}",
            o.slug,
            o.visibility.as_str(),
            o.default_post_visibility.as_str(),
            post_count(c, o.id)?,
            o.name
        );
    }
    Ok(())
}

fn print_show(org: &Org, posts: &[OrgPost], json_out: bool) -> Result<()> {
    if json_out {
        let public = stored_posts_of(org, posts, Audience::Public);
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "slug": org.slug,
                "name": org.name,
                "visibility": org.visibility.as_str(),
                "default_post_visibility": org.default_post_visibility.as_str(),
                "posts": posts.iter().map(|p| json!({
                    "id": p.id,
                    "title": p.title,
                    "body": p.body,
                    "visibility": p.visibility.as_str(),
                    "author": org.slug,
                })).collect::<Vec<_>>(),
                "public_posts": public.len(),
            }))?
        );
        return Ok(());
    }
    println!("{} — {}", org.slug, org.name);
    println!("visibility: {}", org.visibility.as_str());
    println!("default post visibility: {}", org.default_post_visibility.as_str());
    println!("gated: {}", if org.gated { "yes" } else { "no" });
    println!("registration: {}", org.registration.as_str());
    if !org.description.is_empty() {
        println!("{}", org.description);
    }
    if posts.is_empty() {
        println!("No posts yet (--org-post {} --org-title TITLE --body TEXT).", org.slug);
        return Ok(());
    }
    for p in posts {
        println!();
        println!("{}", p.title);
        println!("  id: {}", p.id);
        println!("  author: {}", org.slug);
        println!("  post visibility: {}", p.visibility.as_str());
        if p.archived {
            println!("  archived: yes");
        }
        if !p.body.is_empty() {
            println!("  {}", p.body.replace('\n', "\n  "));
        }
    }
    Ok(())
}

fn stored_posts_of<'a>(org: &Org, posts: &'a [OrgPost], audience: Audience) -> Vec<&'a OrgPost> {
    posts
        .iter()
        .filter(|p| match audience {
            Audience::Owner => true,
            Audience::Public => crate::orgs::public_row(org, p, false),
        })
        .collect()
}

/// Organization posts for the nick bar and search. Empty when the tables are absent.
pub fn posts_for_store(c: &Connection, audience: Audience) -> Result<Vec<Post>> {
    stored_posts(c, audience)
}
