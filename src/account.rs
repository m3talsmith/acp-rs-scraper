//! Profile card, forum settings, and the account writes.
//!
//! `--show-profile` asks the loaded reader, then prints the `profiles` row.
//! `--edit-profile` asks `Edit profile? [y/N]`, then `Type yes`, and calls
//! `update_profile`. `--report-post` and `--report-user` ask the same way and
//! call `report_post` or `report_user`. `--save-settings` asks
//! `Save site settings? [y/N]` and stores `site_state`. There is no settings
//! capability. Web push, a profile photo, and a banner are not built.
//! `--dry-run` prints the action and does not call a service.

use acp_rs_plugin::{AccountRec, CapabilityId, DynService, ServiceId, SourceId, Target};

use crate::prelude::*;

/// Report reasons the confirm UI offers.
pub const REASONS: &[&str] = &["constitution", "illegal_obscene", "spam", "uncomradely"];

/// Profile photo and banner uploads are not built.
pub const PHOTO_NOT_BUILT: &str = "Profile photo and banner uploads are not built.";

/// Web push is not built.
pub const WEB_PUSH_NOT_BUILT: &str = "Web push is not built.";

/// These flags are the account commands.
pub fn cli_requested(args: &Args) -> bool {
    args.show_profile.is_some()
        || args.site_settings
        || args.edit_profile
        || args.save_settings
        || args.report_post.is_some()
        || args.report_user.is_some()
}

/// `--dry-run` text. No service call.
pub fn dry_text(args: &Args) -> Result<String> {
    let body = if let Some(nick) = args.show_profile.as_deref() {
        format!("show-profile {}\n{}", bare_nick(nick)?, PHOTO_NOT_BUILT)
    } else if args.site_settings {
        format!("site-settings\n{WEB_PUSH_NOT_BUILT}")
    } else if args.edit_profile {
        let (name, user, bio) = profile_fields(args)?;
        format!("edit-profile\ndisplay_name: {name}\nusername: {user}\nbio: {bio}\n{PHOTO_NOT_BUILT}")
    } else if args.save_settings {
        let (zone, theme, sustainer) = settings_fields(args)?;
        format!("save-settings\ntime_zone: {zone}\nsite_theme: {theme}\nshow_sustainer: {sustainer}\n{WEB_PUSH_NOT_BUILT}")
    } else if let Some(id) = args.report_post.as_deref() {
        format!("report-post {id}\nreason: {}", reason(args)?)
    } else if let Some(nick) = args.report_user.as_deref() {
        format!("report-user {}\nreason: {}", bare_nick(nick)?, reason(args)?)
    } else {
        bail!("an account command needs a profile, settings, or a report");
    };
    Ok(format!("dry-run: no connection opened\n{body}\n"))
}

/// Ask, then call or store. A dry-run returns before the prompt.
pub async fn run(args: &Args) -> Result<()> {
    let text = dry_text(args)?;
    if args.dry_run {
        println!("{text}");
        return Ok(());
    }
    if args.show_profile.is_some() {
        return show_profile(args).await;
    }
    if args.site_settings {
        println!("{}", settings_line(&home()?.join(DB_REL_PATH))?);
        return Ok(());
    }
    if !crate::prefs::writes_enabled() {
        bail!("{}", crate::actions::WRITES_OFF);
    }
    if !confirmed(args)? {
        println!("nothing sent");
        return Ok(());
    }
    if args.save_settings {
        save_settings(args)?;
        println!("saved site settings");
        return Ok(());
    }
    let cap = if args.edit_profile {
        CapabilityId::UpdateProfile
    } else if args.report_post.is_some() {
        CapabilityId::ReportPost
    } else {
        CapabilityId::ReportUser
    };
    let Some(found) = crate::plugins::live(cap) else {
        bail!("{}", crate::service::SERVICE_ABSENT);
    };
    println!("{}", call(found.get(), args).await?);
    Ok(())
}

async fn show_profile(args: &Args) -> Result<()> {
    let nick = bare_nick(args.show_profile.as_deref().unwrap_or(""))?;
    let Some(found) = crate::plugins::live(CapabilityId::Reader) else {
        bail!("{}", crate::service::SERVICE_ABSENT);
    };
    let service = found.get();
    let resolved = service
        .resolve(&Target::Account { handle: nick.clone() })
        .await
        .map_err(|err| anyhow!(err.to_string()))?;
    let path = home()?.join(DB_REL_PATH);
    println!("{}", profile_line(&path, &nick, &resolved.source_id.native)?);
    Ok(())
}

async fn call(service: &dyn DynService, args: &Args) -> Result<String> {
    if args.edit_profile {
        let (name, user, bio) = profile_fields(args)?;
        let nick = if user.is_empty() { name.clone() } else { user.clone() };
        if nick.is_empty() {
            bail!("an edit needs --display-name, --username, or --bio. Nothing was sent.");
        }
        let mut account = AccountRec::new(SourceId::new(ServiceId::new(service.id()), &nick), nick.clone());
        account.display_name = name;
        account.nick = if user.is_empty() { nick.clone() } else { user };
        account.bio = bio;
        let saved = service.update_profile(&account).await.map_err(|err| anyhow!(err.to_string()))?;
        let path = home()?.join(DB_REL_PATH);
        crate::storage::db::save_profile(&path, &profile_of(&saved))?;
        return Ok(format!("updated profile {}", saved.nick));
    }
    if let Some(id) = args.report_post.as_deref() {
        let id = id.trim();
        if id.is_empty() {
            bail!("a report needs a post id");
        }
        let source = SourceId::new(ServiceId::new(service.id()), id);
        let why = reason(args)?;
        service.report_post(&source, &why).await.map_err(|err| anyhow!(err.to_string()))?;
        return Ok(format!("reported post {id} reason {why}"));
    }
    let nick = bare_nick(args.report_user.as_deref().unwrap_or(""))?;
    let source = SourceId::new(ServiceId::new(service.id()), &nick);
    let why = reason(args)?;
    service.report_user(&source, &why).await.map_err(|err| anyhow!(err.to_string()))?;
    Ok(format!("reported user {nick} reason {why}"))
}

/// A profile row from the account the service returned. Photo and banner stay empty.
pub fn profile_of(account: &AccountRec) -> Profile {
    Profile {
        nick: account.nick.trim_start_matches('@').to_string(),
        user_id: Some(account.source_id.native.clone()).filter(|id| !id.is_empty()),
        display_name: Some(account.display_name.clone()).filter(|s| !s.is_empty()),
        bio: Some(account.bio.clone()).filter(|s| !s.is_empty()),
        avatar_url: Some(account.avatar_url.clone()).filter(|s| !s.is_empty()),
        location: Some(account.location.clone()).filter(|s| !s.is_empty()),
        joined_at: account.joined_at.clone(),
        followers: account.followers,
        following: account.following,
        posts_count: account.posts_count,
        raw_json: String::new(),
        scraped_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        ..Profile::default()
    }
}

fn profile_fields(args: &Args) -> Result<(String, String, String)> {
    let name = args.display_name.clone().unwrap_or_default();
    let user = bare_nick(args.username.as_deref().unwrap_or(""))?;
    let bio = args.bio.clone().unwrap_or_default();
    if name.trim().is_empty() && user.is_empty() && bio.trim().is_empty() {
        bail!("an edit needs --display-name, --username, or --bio. Nothing was sent.");
    }
    Ok((name, user, bio))
}

fn settings_fields(args: &Args) -> Result<(String, String, String)> {
    let zone = args.time_zone.clone().unwrap_or_default();
    let theme = args.site_theme.clone().unwrap_or_default();
    let sustainer = args.show_sustainer.clone().unwrap_or_default();
    if zone.trim().is_empty() && theme.trim().is_empty() && sustainer.trim().is_empty() {
        bail!("settings need --time-zone, --site-theme, or --show-sustainer. Nothing was sent.");
    }
    if !theme.is_empty() && !matches!(theme.as_str(), "light" | "dark" | "system") {
        bail!("--site-theme is light, dark, or system. This is not --theme.");
    }
    Ok((zone, theme, sustainer))
}

fn reason(args: &Args) -> Result<String> {
    let why = args.reason.clone().unwrap_or_default();
    if !REASONS.contains(&why.as_str()) {
        bail!("--reason is constitution, illegal_obscene, spam, or uncomradely");
    }
    Ok(why)
}

fn bare_nick(nick: &str) -> Result<String> {
    let nick = nick.trim().trim_start_matches('@').trim().to_string();
    if nick.is_empty() {
        return Ok(String::new());
    }
    if nick.contains('@') || nick.chars().any(char::is_whitespace) {
        bail!("a username has no @ and no spaces");
    }
    Ok(nick)
}

fn save_settings(args: &Args) -> Result<()> {
    let (zone, theme, sustainer) = settings_fields(args)?;
    let path = home()?.join(DB_REL_PATH);
    let c = open_db(&path)?;
    if !zone.trim().is_empty() {
        crate::storage::db::site_state_set(&c, "time_zone", zone.trim())?;
    }
    if !theme.trim().is_empty() {
        crate::storage::db::site_state_set(&c, "site_theme", theme.trim())?;
    }
    if !sustainer.trim().is_empty() {
        crate::storage::db::site_state_set(&c, "show_sustainer", sustainer.trim())?;
    }
    Ok(())
}

/// Text for a stored profile. Missing fields stay blank. Photo and banner are not printed.
pub fn profile_line(db: &Path, nick: &str, resolved: &str) -> Result<String> {
    let c = open_db(db)?;
    c.execute_batch(PROFILES_SCHEMA)?;
    let row = c
        .query_row(
            "SELECT display_name, bio FROM profiles WHERE nick = ?1",
            params![nick],
            |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get::<_, Option<String>>(1)?.unwrap_or_default())),
        )
        .optional()?;
    let (name, bio) = row.unwrap_or_default();
    Ok(format!("profile {nick}\nid {resolved}\ndisplay name: {name}\nusername: {nick}\nbio: {bio}\n{PHOTO_NOT_BUILT}"))
}

/// Text for stored forum settings.
pub fn settings_line(db: &Path) -> Result<String> {
    let c = open_db(db)?;
    let zone = crate::storage::db::site_state_get(&c, "time_zone")?.unwrap_or_default();
    let theme = crate::storage::db::site_state_get(&c, "site_theme")?.unwrap_or_default();
    let sustainer = crate::storage::db::site_state_get(&c, "show_sustainer")?.unwrap_or_default();
    Ok(format!("site-settings\ntime_zone: {zone}\nsite_theme: {theme}\nshow_sustainer: {sustainer}\n{WEB_PUSH_NOT_BUILT}"))
}

fn confirmed(args: &Args) -> Result<bool> {
    if args.yes {
        return Ok(true);
    }
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(false);
    }
    let (prompt, strict) = if args.edit_profile {
        ("Edit profile? [y/N] ", true)
    } else if args.save_settings {
        ("Save site settings? [y/N] ", false)
    } else if args.report_post.is_some() {
        ("Report post? [y/N] ", true)
    } else {
        ("Report user? [y/N] ", true)
    };
    eprint!("{prompt}");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let yes = matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    if !yes {
        return Ok(false);
    }
    if strict {
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
    use clap::Parser;

    fn args_of(argv: &[&str]) -> Args {
        let mut all = vec!["acp-red-star"];
        all.extend_from_slice(argv);
        Args::parse_from(all)
    }

    #[test]
    fn dry_run_names_the_action_and_does_not_mention_the_site() {
        let edit = args_of(&["--edit-profile", "--display-name", "Ada", "--username", "@ada", "--dry-run"]);
        let text = dry_text(&edit).unwrap();
        assert!(text.contains("no connection opened"));
        assert!(text.contains("username: ada"));
        assert!(text.contains(PHOTO_NOT_BUILT));
        assert!(!text.contains('@'));
        assert!(!text.contains("https://acp.us"));
        let report = args_of(&["--report-post", "POST1", "--reason", "spam", "--dry-run"]);
        let text = dry_text(&report).unwrap();
        assert!(text.contains("report-post POST1"));
        assert!(text.contains("reason: spam"));
        let settings = args_of(&["--save-settings", "--site-theme", "dark", "--show-sustainer", "false", "--dry-run"]);
        let text = dry_text(&settings).unwrap();
        assert!(text.contains("site_theme: dark"));
        assert!(text.contains(WEB_PUSH_NOT_BUILT));
    }

    #[test]
    fn a_returned_account_fills_the_profile_row_without_a_banner() {
        let mut account = AccountRec::new(SourceId::new(ServiceId::new("redstar"), "U1"), "ada");
        account.display_name = "Ada".into();
        account.bio = "organizer".into();
        account.avatar_url = "https://example.test/a.png".into();
        let row = profile_of(&account);
        assert_eq!(row.nick, "ada");
        assert_eq!(row.display_name.as_deref(), Some("Ada"));
        assert!(row.banner_url.is_none());
        assert!(row.banner_path.is_none());
        assert!(row.raw_json.is_empty());
        let dir = std::env::temp_dir().join(format!("acp-account-{}-profile", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rs.database");
        crate::storage::db::save_profile(&path, &row).unwrap();
        let line = profile_line(&path, "ada", "U1").unwrap();
        assert!(line.contains("display name: Ada"));
        assert!(line.contains("bio: organizer"));
        assert!(line.contains(PHOTO_NOT_BUILT));
        let c = open_db(&path).unwrap();
        crate::storage::db::site_state_set(&c, "time_zone", "America/Chicago").unwrap();
        crate::storage::db::site_state_set(&c, "site_theme", "system").unwrap();
        crate::storage::db::site_state_set(&c, "show_sustainer", "true").unwrap();
        let settings = settings_line(&path).unwrap();
        assert!(settings.contains("time_zone: America/Chicago"));
        assert!(settings.contains("site_theme: system"));
        assert!(settings.contains(WEB_PUSH_NOT_BUILT));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
