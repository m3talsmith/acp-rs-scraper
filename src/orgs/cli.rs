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
        || !args.org_export.is_empty()
        || args.org_serve.is_some()
        || args.org_serve_addr.is_some()
        || args.org_publish.is_some()
        || args.org_publish_addr.is_some()
        || args.org_unpublish.is_some()
        || args.org_allow_posts.is_some()
        || args.org_refuse_posts.is_some()
        || args.org_add.is_some()
        || args.org_services
        || args.org_browse.is_some()
        || args.org_identity
        || args.org_invite.is_some()
        || args.role.is_some()
        || args.org_invite_out.is_some()
        || args.org_invite_accept.is_some()
        || args.org_invite_reply.is_some()
        || args.org_fingerprint.is_some()
        || args.org_invite_finish.is_some()
        || args.org_invite_serve.is_some()
        || !args.org_role_create.is_empty()
        || args.rank.is_some()
        || args.permissions.is_some()
        || !args.org_role_assign.is_empty()
        || !args.org_member_remove.is_empty()
        || !args.org_default_register_role.is_empty()
        || !args.org_default_invite_role.is_empty()
        || args.org_archive.is_some()
        || args.org_gate.is_some()
        || args.org_ungate.is_some()
        || !args.org_registration.is_empty()
        || args.org_registrations.is_some()
        || !args.org_registration_approve.is_empty()
        || !args.org_registration_reject.is_empty()
        || args.org_login.is_some()
        || args.org_open_code.is_some()
        || args.org_member_export
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
        println!("register default: {}", org.default_register_role);
        println!("invite default: {}", org.default_invite_role);
        let owners = crate::orgs::access::members(&c, org.id)?.into_iter().filter(|m| m.role.eq_ignore_ascii_case("Owner") && m.status == crate::orgs::access::MemberStatus::Active).count();
        for role in crate::orgs::access::roles(&c, org.id)? {
            let last = if role.name.eq_ignore_ascii_case("Owner") && owners <= 1 { "  last owner" } else { "" };
            println!("{}  rank {}  {}{last}", role.name, role.rank, role.permissions);
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
    if args.org_export.len() == 2 {
        return export_site(args, &db);
    }
    if let Some(slug) = &args.org_serve {
        return serve_site(args, &db, slug);
    }
    if let Some(slug) = &args.org_publish {
        return publish_org(args, &db, slug);
    }
    if let Some(slug) = &args.org_unpublish {
        return unpublish_org(args, &db, slug);
    }
    if let Some(slug) = &args.org_allow_posts {
        return remote_posts(args, &db, slug, true);
    }
    if let Some(slug) = &args.org_refuse_posts {
        return remote_posts(args, &db, slug, false);
    }
    if let Some(url) = &args.org_add {
        return add_org(args, url);
    }
    if args.org_services {
        return list_services();
    }
    if let Some(slug) = &args.org_browse {
        return browse_org(slug);
    }
    if args.org_identity {
        return show_identity(&db);
    }
    if let Some(slug) = &args.org_invite {
        return invite_org(args, &db, slug);
    }
    if let Some(path) = &args.org_invite_accept {
        return accept_org(args, &db, path);
    }
    if let Some(path) = &args.org_invite_finish {
        return finish_org(args, &db, path);
    }
    if args.org_role_create.len() == 2 {
        return create_role(args, &db);
    }
    if args.org_role_assign.len() == 3 {
        return assign_role_cli(args, &db);
    }
    if args.org_member_remove.len() == 2 {
        return remove_member_cli(args, &db);
    }
    if args.org_default_register_role.len() == 2 {
        return default_role_cli(args, &db, crate::orgs::access::Join::Register);
    }
    if args.org_default_invite_role.len() == 2 {
        return default_role_cli(args, &db, crate::orgs::access::Join::Invite);
    }
    if let Some(id) = &args.org_archive {
        return archive_post_cli(args, &db, id);
    }
    if let Some(slug) = &args.org_gate {
        return set_gate(args, &db, slug, true);
    }
    if let Some(slug) = &args.org_ungate {
        return set_gate(args, &db, slug, false);
    }
    if args.org_registration.len() == 2 {
        return set_registration_cli(args, &db);
    }
    if let Some(slug) = &args.org_registrations {
        return list_registrations(&db, slug);
    }
    if args.org_registration_approve.len() == 2 {
        return registration_decision(args, &db, true);
    }
    if args.org_registration_reject.len() == 2 {
        return registration_decision(args, &db, false);
    }
    if let Some(url) = &args.org_login {
        return login_remote(args, &db, url);
    }
    if let Some(code) = &args.org_open_code {
        return open_code(args, &db, code);
    }
    if let Some(slug) = &args.org_post {
        return post(args, &db, slug);
    }
    bail!("--org-name, --org-description, --org-visibility, --org-default-visibility, --org-title, --org-post-visibility, --role, --rank, --permissions, and --org-fingerprint belong to an organization command");
}

fn create_role(args: &Args, db: &Path) -> Result<()> {
    let slug = &args.org_role_create[0];
    let name = &args.org_role_create[1];
    let rank = args.rank.ok_or_else(|| anyhow!("--org-role-create needs --rank"))?;
    let list = args.permissions.as_deref().ok_or_else(|| anyhow!("--org-role-create needs --permissions"))?;
    let perms: Vec<&str> = list.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    if args.dry_run {
        println!("would create role {name} at rank {rank} on {slug}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let role = crate::orgs::access::create_custom_role(&c, org.id, &actor, name, rank, &perms)?;
    println!("{}  rank {}  {}", role.name, role.rank, role.permissions);
    Ok(())
}

fn assign_role_cli(args: &Args, db: &Path) -> Result<()> {
    let slug = &args.org_role_assign[0];
    let member = &args.org_role_assign[1];
    let role = &args.org_role_assign[2];
    if args.dry_run {
        println!("would assign {role} to {member} in {slug}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let updated = crate::orgs::access::assign_role(&c, org.id, &actor, member, role)?;
    println!("{}  {}  {}", updated.identity_pub, updated.role, updated.status.as_str());
    Ok(())
}

fn remove_member_cli(args: &Args, db: &Path) -> Result<()> {
    let slug = &args.org_member_remove[0];
    let member = &args.org_member_remove[1];
    if args.dry_run {
        println!("would remove member {member} from {slug}");
        println!("their posts stay");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    println!("Removing a member leaves their posts. Cancel is the default.");
    if !confirm(args, "Remove this member? [y/N] ")? {
        println!("cancelled");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    crate::orgs::access::remove_member(&c, org.id, &actor, member)?;
    println!("removed {member}");
    println!("their posts stay");
    Ok(())
}

fn default_role_cli(args: &Args, db: &Path, join: crate::orgs::access::Join) -> Result<()> {
    let (slug, role, label) = match join {
        crate::orgs::access::Join::Register => (&args.org_default_register_role[0], &args.org_default_register_role[1], "register"),
        crate::orgs::access::Join::Invite => (&args.org_default_invite_role[0], &args.org_default_invite_role[1], "invite"),
    };
    if args.dry_run {
        println!("would set the {label} default to {role} on {slug}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    crate::orgs::access::set_default_role(&c, org.id, join, role)?;
    println!("{label} default: {role}");
    Ok(())
}

fn archive_post_cli(args: &Args, db: &Path, id: &str) -> Result<()> {
    if args.dry_run {
        println!("would archive {id}");
        println!("kind: update");
        println!("not a delete");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    println!("Archiving keeps the row and drops it from public pages. Cancel is the default.");
    if !confirm(args, "Archive this post? [y/N] ")? {
        println!("cancelled");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let post = crate::orgs::get_post(&c, id)?.ok_or_else(|| anyhow!("no post {id}"))?;
    let record = crate::orgs::access::archive_post(&c, post.org_id, &actor, id)?;
    println!("archived {id}");
    println!("kind: {}", record.kind);
    println!("not a delete");
    Ok(())
}

fn set_gate(args: &Args, db: &Path, slug: &str, gated: bool) -> Result<()> {
    let preview = crate::orgs::gate::gate_preview(gated);
    let verb = if gated { "gate" } else { "ungate" };
    if args.dry_run {
        println!("would {verb} {slug}");
        println!("{preview}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    println!("{preview}");
    let prompt = if gated { "Hide public rows from non-members? [y/N] " } else { "Let non-members see public rows? [y/N] " };
    if !confirm(args, prompt)? {
        println!("cancelled");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    crate::orgs::access::require(&c, org.id, &actor, "manage_access")?;
    crate::orgs::access::set_gated(&c, org.id, gated)?;
    println!("gated: {}", if gated { "yes" } else { "no" });
    Ok(())
}

fn set_registration_cli(args: &Args, db: &Path) -> Result<()> {
    let slug = &args.org_registration[0];
    let mode = crate::orgs::Registration::parse(&args.org_registration[1])?;
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    crate::orgs::access::require(&c, org.id, &actor, "manage_access")?;
    crate::orgs::access::set_registration(&c, org.id, mode)?;
    println!("registration: {}", mode.as_str());
    Ok(())
}

fn list_registrations(db: &Path, slug: &str) -> Result<()> {
    let c = org_conn(db)?;
    let rows = crate::orgs::gate::pending(&c, slug)?;
    if rows.is_empty() {
        println!("No pending registrations");
        return Ok(());
    }
    for (identity, fingerprint) in rows {
        println!("{identity}  fingerprint: {fingerprint}");
    }
    Ok(())
}

fn registration_decision(args: &Args, db: &Path, approve: bool) -> Result<()> {
    let (slug, member) = if approve {
        (&args.org_registration_approve[0], &args.org_registration_approve[1])
    } else {
        (&args.org_registration_reject[0], &args.org_registration_reject[1])
    };
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let c = org_conn(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    if approve {
        let updated = crate::orgs::access::approve_registration(&c, org.id, &actor, member)?;
        println!("{}  {}  {}", updated.identity_pub, updated.role, updated.status.as_str());
    } else {
        crate::orgs::access::reject_registration(&c, org.id, &actor, member)?;
        println!("rejected {member}");
    }
    Ok(())
}

fn open_code(args: &Args, db: &Path, code: &str) -> Result<()> {
    let c = org_conn(db)?;
    let view = crate::orgs::gate::describe_code(&c, code)?;
    let service = crate::orgs::gate::service_identity(db, &view.slug)?;
    println!("name: {}", view.name);
    println!("origin: this machine");
    println!("fingerprint: {}", service.fingerprint());
    if args.dry_run {
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Send this reply? [y/N] ")? {
        println!("cancelled");
        return Ok(());
    }
    let me = crate::orgs::access::local_identity(db)?;
    if view.kind == "register" {
        let invite = view.invite.ok_or_else(|| anyhow!("registration has no invite"))?;
        let typed = crate::orgs::members::shown_fingerprint(&me, &invite)?;
        let (answered, _) = crate::orgs::members::accept_invite(&invite, &me, &typed)?;
        let member = crate::orgs::gate::complete_register(&c, &service, &answered, &typed, &view.browser, &view.csrf)?;
        println!("{}  {}  {}", member.identity_pub, member.role, member.status.as_str());
    } else {
        let signature = me.sign(&crate::orgs::gate::login_message(&view.nonce, &view.slug, code));
        let session = crate::orgs::gate::complete_login(&c, &view.slug, code, &view.browser, &view.csrf, &crate::orgs::access::pub_hex(&me), &signature)?;
        println!("session: {session}");
    }
    Ok(())
}

fn login_remote(args: &Args, db: &Path, url: &str) -> Result<()> {
    let addr = loopback_http(url)?;
    if args.dry_run {
        println!("would sign a login nonce at {url}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    let began = http_exchange(addr, "POST", "/v1/login", "", "")?;
    let fields = json_object(&began)?;
    let code = json_str(&fields, "code")?;
    let nonce = json_str(&fields, "nonce")?;
    let csrf = json_str(&fields, "csrf")?;
    let slug = json_str(&fields, "slug")?;
    let cookie = set_cookie(&began)?;
    let me = crate::orgs::access::local_identity(db)?;
    let signature = data_encoding::HEXLOWER.encode(&me.sign(&crate::orgs::gate::login_message(&nonce, &slug, &code)));
    let body = serde_json::json!({
        "code": code,
        "csrf": csrf,
        "identity": crate::orgs::access::pub_hex(&me),
        "signature": signature,
    })
    .to_string();
    let done = http_exchange(addr, "POST", "/v1/login/finish", &format!("Cookie: {cookie}\r\n"), &body)?;
    let session = json_str(&json_object(&done)?, "session")?;
    println!("session: {session}");
    Ok(())
}

fn loopback_http(raw: &str) -> Result<std::net::SocketAddr> {
    let rest = raw.strip_prefix("http://").ok_or_else(|| anyhow!("organization login uses http://127.0.0.1"))?;
    let hostport = rest.split('/').next().unwrap_or(rest);
    let addr: std::net::SocketAddr = hostport.parse().with_context(|| format!("parsing {raw}"))?;
    crate::orgs::members::require_loopback(addr)?;
    Ok(addr)
}

fn http_exchange(addr: std::net::SocketAddr, method: &str, path: &str, extra: &str, body: &str) -> Result<String> {
    let mut sock = std::net::TcpStream::connect(addr).with_context(|| format!("connecting to {addr}"))?;
    let req = format!("{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    std::io::Write::write_all(&mut sock, req.as_bytes())?;
    let mut out = String::new();
    std::io::Read::read_to_string(&mut sock, &mut out)?;
    if !out.contains("200") {
        bail!("{}", out.split("\r\n\r\n").nth(1).unwrap_or(out.trim()));
    }
    Ok(out)
}

fn json_object(raw: &str) -> Result<serde_json::Value> {
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or(raw);
    Ok(serde_json::from_str(body)?)
}

fn json_str(v: &serde_json::Value, key: &str) -> Result<String> {
    v.get(key).and_then(|s| s.as_str()).map(str::to_string).ok_or_else(|| anyhow!("login response has no {key}"))
}

fn set_cookie(raw: &str) -> Result<String> {
    let line = raw.lines().find(|l| l.to_ascii_lowercase().starts_with("set-cookie:")).ok_or_else(|| anyhow!("login response has no cookie"))?;
    let value = line.split(':').nth(1).unwrap_or("").split(';').next().unwrap_or("").trim();
    if !value.starts_with("acp_org=") {
        bail!("login cookie is not a session id");
    }
    Ok(value.to_string())
}

fn site_names(site: &crate::orgs::site::Site) -> Vec<String> {
    let mut names = vec!["index.html".into(), "feed.json".into()];
    for post in &site.posts {
        names.push(format!("posts/{}.html", post.id));
        names.push(format!("posts/{}.md", post.id));
    }
    for file in &site.files {
        names.push(format!("downloads/{}.html", file.id));
        if !file.bytes.is_empty() {
            names.push(format!("downloads/{}", file.id));
        }
    }
    names
}

fn show_identity(db: &Path) -> Result<()> {
    let secret = crate::orgs::access::local_identity(db)?;
    println!("fingerprint: {}", secret.fingerprint());
    Ok(())
}

fn invite_org(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let role = args.role.as_deref().ok_or_else(|| anyhow!("--org-invite needs --role"))?;
    if args.org_invite_out.is_none() && args.org_invite_serve.is_none() {
        bail!("--org-invite needs --org-invite-out or --org-invite-serve");
    }
    let serve = if let Some(raw) = &args.org_invite_serve {
        let addr: std::net::SocketAddr = raw.parse().with_context(|| format!("parsing {raw}"))?;
        crate::orgs::members::require_loopback(addr)?;
        Some(addr)
    } else {
        None
    };
    if args.dry_run {
        println!("would invite a {role} to {slug}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    let inviter = crate::orgs::access::local_identity(db)?;
    let c = org_conn(db)?;
    let invite = crate::orgs::members::create_invite(&c, slug, &inviter, role, crate::orgs::members::INVITE_TTL_SECS)?;
    if let Some(addr) = serve {
        let (listener, bound) = crate::orgs::members::listen_invite(addr)?;
        println!("listening: {bound}");
        let line = serde_json::to_string(&invite)?;
        let reply = crate::orgs::members::send_invite(listener, &line)?;
        let answered = crate::orgs::members::from_json(&reply)?;
        let fp = crate::orgs::members::shown_fingerprint(&inviter, &answered)?;
        println!("fingerprint: {fp}");
        let typed = args.org_fingerprint.as_deref().unwrap_or("");
        crate::orgs::members::finish_invite(&c, &answered, &inviter, typed)?;
        println!("member added");
        println!("role: {}", answered.role);
    }
    if let Some(path) = &args.org_invite_out {
        crate::orgs::members::write_invite(Path::new(path), &invite)?;
        println!("invite written");
        println!("role: {}", invite.role);
    }
    Ok(())
}

fn accept_org(args: &Args, db: &Path, path: &str) -> Result<()> {
    let reply = args.org_invite_reply.as_deref().ok_or_else(|| anyhow!("--org-invite-accept needs --org-invite-reply"))?;
    let invite = crate::orgs::members::read_invite(Path::new(path))?;
    let invitee = crate::orgs::access::local_identity(db)?;
    let fp = crate::orgs::members::shown_fingerprint(&invitee, &invite)?;
    println!("fingerprint: {fp}");
    if args.dry_run {
        println!("would accept the invite for {}", invite.slug);
        println!("dry-run: nothing was written");
        return Ok(());
    }
    let typed = args.org_fingerprint.as_deref().unwrap_or("");
    let (answered, _) = crate::orgs::members::accept_invite(&invite, &invitee, typed)?;
    crate::orgs::members::write_invite(Path::new(reply), &answered)?;
    println!("invite accepted");
    Ok(())
}

fn finish_org(args: &Args, db: &Path, path: &str) -> Result<()> {
    let invite = crate::orgs::members::read_invite(Path::new(path))?;
    let inviter = crate::orgs::access::local_identity(db)?;
    let fp = crate::orgs::members::shown_fingerprint(&inviter, &invite)?;
    println!("fingerprint: {fp}");
    if args.dry_run {
        println!("would add the member to {}", invite.slug);
        println!("dry-run: nothing was written");
        return Ok(());
    }
    let typed = args.org_fingerprint.as_deref().unwrap_or("");
    crate::orgs::members::finish_invite(&org_conn(db)?, &invite, &inviter, typed)?;
    println!("member added");
    println!("role: {}", invite.role);
    Ok(())
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

fn export_site(args: &Args, db: &Path) -> Result<()> {
    let slug = &args.org_export[0];
    let dir = PathBuf::from(&args.org_export[1]);
    let member = args.org_member_export;
    let preview = if member {
        format!(
            "would export the member site for {slug} to {}\nThis folder is member content and must not be published as the open site.",
            dir.display()
        )
    } else {
        format!("would export the public site for {slug} to {}", dir.display())
    };
    if args.dry_run {
        println!("{preview}");
        if member {
            let c = org_conn(db)?;
            let site = crate::orgs::site::build_for(&c, db, slug, true)?;
            for name in site_names(&site) {
                println!("{name}");
            }
        }
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if member {
        println!("This folder is member content and must not be published as the open site.");
    }
    let prompt = if member { "Write member content? [y/N] " } else { "Write the public site? [y/N] " };
    if !confirm(args, prompt)? {
        println!("{preview}");
        println!("cancelled");
        return Ok(());
    }
    let c = org_conn(db)?;
    let site = if member { crate::orgs::site::build_for(&c, db, slug, true)? } else { crate::orgs::site::build(&c, db, slug)? };
    crate::orgs::site::write(&site, &dir)?;
    println!("exported {slug} to {}", dir.display());
    println!("{}", site.note);
    println!("public posts: {}", site.posts.len());
    println!("left out: {}", site.omitted);
    println!("{}", crate::orgs::site::REPLICATION_NOTE);
    Ok(())
}

fn serve_site(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let raw = args.org_serve_addr.as_deref().unwrap_or("127.0.0.1:0");
    let addr: std::net::SocketAddr = raw.parse().with_context(|| format!("bad address {raw}"))?;
    let c = org_conn(db)?;
    let site = crate::orgs::site::build(&c, db, slug)?;
    crate::orgs::site::serve(&site, addr)?;
    Ok(())
}

fn services_dir() -> Result<PathBuf> {
    Ok(home()?.join(".acp/services"))
}

fn publish_org(args: &Args, db: &Path, slug: &str) -> Result<()> {
    let c = org_conn(db)?;
    let text = crate::orgs::service::preview(&c, slug)?;
    if args.dry_run {
        println!("would publish {slug}");
        println!("{text}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Publish this organization? [y/N] ")? {
        println!("would publish {slug}");
        println!("cancelled");
        return Ok(());
    }
    let actor = crate::orgs::access::local_identity_pub(db)?;
    let org = get_org(&c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    crate::orgs::access::require(&c, org.id, &actor, "publish")?;
    let raw = args.org_publish_addr.as_deref().unwrap_or("127.0.0.1:0");
    let addr: std::net::SocketAddr = raw.parse().with_context(|| format!("bad address {raw}"))?;
    let stop = std::sync::atomic::AtomicBool::new(false);
    let (tx, _rx) = std::sync::mpsc::channel();
    crate::orgs::service::serve_published(db, &services_dir()?, slug, addr, &tx, &stop)?;
    Ok(())
}

fn unpublish_org(args: &Args, db: &Path, slug: &str) -> Result<()> {
    if args.dry_run {
        println!("would withdraw {slug}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Withdraw this organization? [y/N] ")? {
        println!("would withdraw {slug}");
        println!("cancelled");
        return Ok(());
    }
    let c = org_conn(db)?;
    crate::orgs::service::unpublish(&c, &services_dir()?, slug)?;
    println!("withdrew {slug}");
    Ok(())
}

fn remote_posts(args: &Args, db: &Path, slug: &str, on: bool) -> Result<()> {
    let verb = if on { "allow" } else { "refuse" };
    if args.dry_run {
        println!("would {verb} remote posts for {slug}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Change remote posts? [y/N] ")? {
        println!("would {verb} remote posts for {slug}");
        println!("cancelled");
        return Ok(());
    }
    let c = org_conn(db)?;
    crate::orgs::service::set_remote_posts(&c, &services_dir()?, slug, on)?;
    println!("remote posts for {slug}: {}", if on { "on" } else { "off" });
    Ok(())
}

fn add_org(args: &Args, url: &str) -> Result<()> {
    if args.dry_run {
        println!("would add {url}");
        println!("dry-run: nothing was written");
        return Ok(());
    }
    if !confirm(args, "Add this organization service? [y/N] ")? {
        println!("would add {url}");
        println!("cancelled");
        return Ok(());
    }
    let slug = crate::orgs::service::add_remote(&services_dir()?, url)?;
    println!("added {slug}");
    Ok(())
}

fn list_services() -> Result<()> {
    let dir = services_dir()?;
    let mut names = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".toml") {
                names.push(name);
            }
        }
    }
    names.sort();
    if names.is_empty() {
        println!("No organization services yet");
        return Ok(());
    }
    for name in names {
        println!("{name}");
    }
    Ok(())
}

fn browse_org(slug: &str) -> Result<()> {
    let body = crate::orgs::service::browse(&services_dir()?, slug, &[])?;
    println!("{body}");
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
