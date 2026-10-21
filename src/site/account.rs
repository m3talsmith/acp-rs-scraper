//! Profile card, forum settings, and the gated account writes.
//!
//! Viewing a profile and `/rs/settings` is shipped. Editing a profile, saving
//! settings, reporting a post, and reporting a user preview the documented
//! form and do not open a connection.

use super::write::WriteError;
use crate::prelude::*;
use crate::service::{Availability, CapabilityId, RedStar};

const FIXTURE: &str = include_str!("../../tests/fixtures/site/account.json");

/// Reasons the report forms document.
pub const REPORT_REASONS: [&str; 4] = ["constitution", "illegal_obscene", "spam", "uncomradely"];

/// Sent to the user when a confirmed account write is refused.
pub const ACCOUNT_GATE: &str = "edit profile, site settings, report post, and report user are not sent. The live frames have not been captured. --dry-run prints the form and does not connect.";

/// Fields read from a profile page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileCard {
    /// Nick the page was opened for.
    pub nick: String,
    /// `data-display-name`, or the profile heading.
    pub display_name: String,
    /// `data-username`, or the nick.
    pub username: String,
    /// `data-bio`. Empty when the page has none.
    pub bio: String,
}

/// The three forum settings the page shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteSettings {
    /// `form[time_zone_id]`.
    pub time_zone: String,
    /// `form[theme]`: light, dark, or system.
    pub theme: String,
    /// `form[show_sustainer?]`.
    pub show_sustainer: String,
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("account fixture")
}

fn err(message: impl Into<String>) -> WriteError {
    WriteError { message: message.into() }
}

fn needed() -> String {
    let mut s = String::from("capture still needed:\n");
    for line in fixture()["needed"].as_array().into_iter().flatten() {
        if let Some(line) = line.as_str() {
            s.push_str("- ");
            s.push_str(line);
            s.push('\n');
        }
    }
    s
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn writes_off() -> WriteError {
    err("writes are off. Set ui.writes = true in preferences.toml, or unset ACP_WRITES. ACP_WRITES=off wins. Nothing was sent.")
}

fn finish(dry_run: bool, confirmed: bool, text: String) -> Result<String, WriteError> {
    if dry_run {
        return Ok(text);
    }
    if !confirmed {
        return Ok("nothing sent".into());
    }
    Err(err(format!("{ACCOUNT_GATE}\n\n{text}")))
}

fn attr(el: scraper::ElementRef<'_>, name: &str) -> String {
    el.value().attr(name).unwrap_or("").trim().to_string()
}

/// Read the profile card out of a profile page. No network.
pub fn parse_profile_card(nick: &str, html: &str) -> Result<ProfileCard> {
    let doc = Html::parse_document(html);
    let card_sel = Selector::parse("#js-profileCard").unwrap();
    let name_sel = Selector::parse("h1.ProfileName").unwrap();
    let mut display = String::new();
    let mut username = nick.to_string();
    let mut bio = String::new();
    if let Some(el) = doc.select(&card_sel).next() {
        let name = attr(el, "data-display-name");
        let user = attr(el, "data-username");
        bio = attr(el, "data-bio");
        if !name.is_empty() {
            display = name;
        }
        if !user.is_empty() {
            username = user;
        }
    }
    if display.is_empty()
        && let Some(el) = doc.select(&name_sel).next()
    {
        display = one_line(&el.text().collect::<String>());
    }
    if display.is_empty() {
        bail!("the profile page has no display name");
    }
    Ok(ProfileCard { nick: nick.to_string(), display_name: display, username, bio })
}

fn input_value(doc: &Html, name: &str) -> Option<String> {
    let sel = Selector::parse("input").unwrap();
    for el in doc.select(&sel) {
        if el.value().attr("name") == Some(name) {
            let value = el.value().attr("value").unwrap_or("").trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Read the three settings fields. No network.
pub fn parse_settings(html: &str) -> Result<SiteSettings> {
    let doc = Html::parse_document(html);
    let time_zone = input_value(&doc, "form[time_zone_id]").unwrap_or_default();
    let theme = input_value(&doc, "form[theme]").unwrap_or_default();
    let show_sustainer = input_value(&doc, "form[show_sustainer?]").unwrap_or_default();
    if time_zone.is_empty() || theme.is_empty() || show_sustainer.is_empty() {
        bail!("the settings page is missing a time zone, a theme, or show-sustainer");
    }
    Ok(SiteSettings { time_zone, theme, show_sustainer })
}

fn store_card(card: &ProfileCard) -> Result<()> {
    let profile = Profile {
        nick: card.nick.clone(),
        display_name: Some(card.display_name.clone()),
        bio: Some(card.bio.clone()),
        raw_json: json!({
            "nick": card.nick,
            "username": card.username,
            "display_name": card.display_name,
            "bio": card.bio,
        })
        .to_string(),
        scraped_at: chrono::Utc::now().to_rfc3339(),
        ..Profile::default()
    };
    save_profile(&home()?.join(DB_REL_PATH), &profile)
}

fn store_settings(s: &SiteSettings) -> Result<()> {
    let c = open_db(&home()?.join(DB_REL_PATH))?;
    site_state_set(&c, "site.time_zone", &s.time_zone)?;
    site_state_set(&c, "site.theme", &s.theme)?;
    site_state_set(&c, "site.show_sustainer", &s.show_sustainer)?;
    Ok(())
}

/// `GET /rs/@nick`, store the card, and return the printable line.
pub async fn read_profile(http: &reqwest::Client, nick: &str) -> Result<Vec<String>> {
    RedStar::require(CapabilityId::ProfileRead)?;
    let nick = nick.trim().trim_start_matches('@');
    if nick.is_empty() || nick.contains(char::is_whitespace) || nick.contains('/') {
        bail!("a profile needs a nick");
    }
    let html = http.get(profile_url(nick)).send().await?.error_for_status()?.text().await?;
    let card = parse_profile_card(nick, &html)?;
    store_card(&card)?;
    let line = format!("profile\t{}\t{}\t{}\t{}", card.nick, one_line(&card.display_name), one_line(&card.username), one_line(&card.bio));
    info!("{line}");
    Ok(vec![line])
}

/// `GET /rs/settings`, store the three fields, and return the printable lines.
pub async fn read_settings(http: &reqwest::Client) -> Result<Vec<String>> {
    RedStar::require(CapabilityId::SettingsRead)?;
    let url = format!("{}/rs/settings", base_url());
    let html = http.get(url).send().await?.error_for_status()?.text().await?;
    let s = parse_settings(&html)?;
    store_settings(&s)?;
    let lines = vec![
        format!("site-settings\ttime_zone\t{}", one_line(&s.time_zone)),
        format!("site-settings\ttheme\t{}", one_line(&s.theme)),
        format!("site-settings\tshow_sustainer\t{}", one_line(&s.show_sustainer)),
    ];
    for line in &lines {
        info!("{line}");
    }
    Ok(lines)
}

/// The stored card, for the terminal UI after a profile job.
pub fn load_card(nick: &str) -> Result<Option<ProfileCard>> {
    let path = home()?.join(DB_REL_PATH);
    if !path.exists() { return Ok(None); }
    let c = open_db(&path)?;
    c.execute_batch(PROFILES_SCHEMA)?;
    let mut stmt = c.prepare("SELECT display_name, bio, raw_json FROM profiles WHERE nick = ?1")?;
    let mut rows = stmt.query(rusqlite::params![nick])?;
    let Some(row) = rows.next()? else { return Ok(None) };
    let display_name: Option<String> = row.get(0)?;
    let bio: Option<String> = row.get(1)?;
    let raw: String = row.get(2)?;
    let username = serde_json::from_str::<Value>(&raw).ok().and_then(|v| v.get("username").and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| nick.to_string());
    Ok(Some(ProfileCard { nick: nick.to_string(), display_name: display_name.unwrap_or_default(), username, bio: bio.unwrap_or_default() }))
}

/// The stored forum settings.
pub fn load_settings() -> Result<Option<SiteSettings>> {
    let path = home()?.join(DB_REL_PATH);
    if !path.exists() { return Ok(None); }
    let c = open_db(&path)?;
    c.execute_batch(SITE_STATE_SCHEMA)?;
    let get = |key: &str| -> Result<Option<String>> {
        match c.query_row("SELECT value FROM site_state WHERE key = ?1", rusqlite::params![key], |r| r.get(0)) {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    };
    let (Some(time_zone), Some(theme), Some(show_sustainer)) = (get("site.time_zone")?, get("site.theme")?, get("site.show_sustainer")?) else {
        return Ok(None);
    };
    Ok(Some(SiteSettings { time_zone, theme, show_sustainer }))
}

fn clean_field(label: &str, value: &str, username: bool) -> Result<String, WriteError> {
    let value = one_line(value);
    if value.is_empty() {
        return Err(err(format!("{label} is empty")));
    }
    if username && (value.starts_with('@') || value.contains('/')) {
        return Err(err("a username has no @ and no slash"));
    }
    Ok(value)
}

/// Preview an edit. A confirm does not connect.
pub fn present_profile_edit(display_name: Option<&str>, username: Option<&str>, bio: Option<&str>, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(writes_off());
    }
    if RedStar::known(CapabilityId::UpdateProfile).map_err(|e| err(e.to_string()))? != Availability::Gated {
        return Err(err("edit profile has no sender"));
    }
    if display_name.is_none() && username.is_none() && bio.is_none() {
        return Err(err("an edit needs a display name, a username, or a bio"));
    }
    let mut lines = vec![
        "dry-run: no connection opened".into(),
        "edit-profile".into(),
        "method=put".into(),
        "phx-submit=save".into(),
        "form=edit-profile-form".into(),
    ];
    if let Some(v) = display_name {
        lines.push(format!("form[display_name]={}", clean_field("display name", v, false)?));
    }
    if let Some(v) = username {
        lines.push(format!("form[username]={}", clean_field("username", v, true)?));
    }
    if let Some(v) = bio {
        lines.push(format!("form[forum_profile][bio]={}", clean_field("bio", v, false)?));
    }
    lines.push("hidden _persistent_id and _form_type are not filled until the capture".into());
    lines.push("profile photo and banner uploads are not built".into());
    lines.push(needed());
    finish(dry_run, confirmed, lines.join("\n"))
}

/// Preview a settings save. A confirm does not connect.
pub fn present_settings(time_zone: Option<&str>, theme: Option<&str>, show_sustainer: Option<&str>, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(writes_off());
    }
    if RedStar::known(CapabilityId::UpdateSettings).map_err(|e| err(e.to_string()))? != Availability::Gated {
        return Err(err("site settings have no sender"));
    }
    if time_zone.is_none() && theme.is_none() && show_sustainer.is_none() {
        return Err(err("a save needs a time zone, a theme, or show-sustainer"));
    }
    let mut lines = vec![
        "dry-run: no connection opened".into(),
        "save-settings".into(),
        "form=settings-form".into(),
        "phx-submit=save".into(),
    ];
    if let Some(v) = time_zone {
        let v = clean_field("time zone", v, false)?;
        if v.contains(' ') {
            return Err(err("a time zone has no spaces"));
        }
        lines.push(format!("form[time_zone_id]={v}"));
    }
    if let Some(v) = theme {
        let v = clean_field("theme", v, false)?;
        if !matches!(v.as_str(), "light" | "dark" | "system") {
            return Err(err("theme must be light, dark, or system"));
        }
        lines.push(format!("form[theme]={v}"));
    }
    if let Some(v) = show_sustainer {
        let v = clean_field("show sustainer", v, false)?;
        if !matches!(v.as_str(), "true" | "false") {
            return Err(err("show sustainer must be true or false"));
        }
        lines.push(format!("form[show_sustainer?]={v}"));
    }
    lines.push("web push is not built".into());
    lines.push(needed());
    finish(dry_run, confirmed, lines.join("\n"))
}

fn reason(value: &str) -> Result<String, WriteError> {
    let v = clean_field("reason", value, false)?;
    if !REPORT_REASONS.contains(&v.as_str()) {
        return Err(err("reason must be constitution, illegal_obscene, spam, or uncomradely"));
    }
    Ok(v)
}

/// Preview a post report. `thread` is the thread root. A confirm does not connect.
pub fn present_report_post(id: &str, thread: &str, why: &str, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(writes_off());
    }
    if RedStar::known(CapabilityId::ReportPost).map_err(|e| err(e.to_string()))? != Availability::Gated {
        return Err(err("report post has no sender"));
    }
    let id = clean_field("post id", id, true)?;
    let thread = if thread.trim().is_empty() { id.clone() } else { clean_field("thread id", thread, true)? };
    let why = reason(why)?;
    let text = format!(
        "dry-run: no connection opened\nreport-post {id}\npushEventTo=#js-reportPostActions\nevent=show-actions\npost_id={id}\nthread_id={thread}\nform=js-reportPostActions-form\nphx-submit=save\nform[reason]={why}\n{}",
        needed()
    );
    finish(dry_run, confirmed, text)
}

/// Preview a user report. A confirm does not connect.
pub fn present_report_user(nick: &str, why: &str, confirmed: bool, dry_run: bool) -> Result<String, WriteError> {
    if !super::write::writes_enabled() {
        return Err(writes_off());
    }
    if RedStar::known(CapabilityId::ReportUser).map_err(|e| err(e.to_string()))? != Availability::Gated {
        return Err(err("report user has no sender"));
    }
    let nick = nick.trim().trim_start_matches('@');
    let nick = clean_field("nick", nick, true)?;
    let why = reason(why)?;
    let text = format!(
        "dry-run: no connection opened\nreport-user {nick}\nform=js-reportUserActions-form\ncid=3\nphx-submit=save\nform[reason]={why}\n{}",
        needed()
    );
    finish(dry_run, confirmed, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_card_keeps_the_visible_fields() {
        let html = r#"<div id="js-profileCard" data-username="ada" data-display-name="Ada Example" data-bio="organizing in the shop"></div><h1 class="ProfileName">Ada Example</h1>"#;
        let card = parse_profile_card("ada", html).unwrap();
        assert_eq!(card.display_name, "Ada Example");
        assert_eq!(card.username, "ada");
        assert_eq!(card.bio, "organizing in the shop");
        let html = r#"<h1 class="ProfileName">Ada Example</h1>"#;
        let card = parse_profile_card("ada", html).unwrap();
        assert_eq!(card.display_name, "Ada Example");
        assert_eq!(card.username, "ada");
        assert!(card.bio.is_empty());
    }

    #[test]
    fn settings_keep_the_three_fields() {
        let html = r#"<form id="settings-form"><input name="form[time_zone_id]" value="America/Chicago"><input name="form[theme]" value="dark"><input name="form[show_sustainer?]" value="true"></form>"#;
        let s = parse_settings(html).unwrap();
        assert_eq!(s.time_zone, "America/Chicago");
        assert_eq!(s.theme, "dark");
        assert_eq!(s.show_sustainer, "true");
    }

    #[test]
    fn a_confirmed_edit_is_not_sent() {
        assert_eq!(fixture()["capture_status"], "missing");
        let dry = present_profile_edit(Some("Ada"), Some("ada"), Some("hello"), false, true).unwrap();
        assert!(dry.contains("dry-run: no connection opened"), "{dry}");
        assert!(dry.contains("form=edit-profile-form"), "{dry}");
        assert!(dry.contains("form[display_name]=Ada"), "{dry}");
        assert!(dry.contains("form[username]=ada"), "{dry}");
        assert!(dry.contains("form[forum_profile][bio]=hello"), "{dry}");
        assert!(dry.contains("profile photo and banner uploads are not built"), "{dry}");
        assert!(dry.contains("DevTools open"), "{dry}");
        assert!(!dry.contains("https://acp.us"), "{dry}");
        assert_eq!(present_profile_edit(Some("Ada"), None, None, false, false).unwrap(), "nothing sent");
        let blocked = present_profile_edit(Some("Ada"), None, None, true, false).unwrap_err();
        assert!(blocked.message.contains("are not sent"), "{}", blocked.message);
        assert!(blocked.message.contains("form[display_name]=Ada"), "{}", blocked.message);
        assert!(blocked.message.contains("capture still needed"), "{}", blocked.message);
    }

    #[test]
    fn reports_and_settings_stay_gated() {
        let dry = present_settings(Some("America/Chicago"), Some("dark"), Some("true"), false, true).unwrap();
        assert!(dry.contains("form=settings-form"), "{dry}");
        assert!(dry.contains("form[show_sustainer?]=true"), "{dry}");
        assert!(dry.contains("web push is not built"), "{dry}");
        let blocked = present_settings(None, Some("dark"), None, true, false).unwrap_err();
        assert!(blocked.message.contains("are not sent"), "{}", blocked.message);
        let dry = present_report_post("POST", "THREAD", "spam", false, true).unwrap();
        assert!(dry.contains("pushEventTo=#js-reportPostActions"), "{dry}");
        assert!(dry.contains("thread_id=THREAD"), "{dry}");
        assert!(dry.contains("form[reason]=spam"), "{dry}");
        assert!(present_report_post("POST", "THREAD", "nope", true, true).is_err());
        let dry = present_report_user("@ada", "uncomradely", false, true).unwrap();
        assert!(dry.contains("report-user ada"), "{dry}");
        assert!(dry.contains("form=js-reportUserActions-form"), "{dry}");
        assert!(dry.contains("cid=3"), "{dry}");
        let blocked = present_report_user("ada", "spam", true, false).unwrap_err();
        assert!(blocked.message.contains("are not sent"), "{}", blocked.message);
        assert!(!blocked.message.contains("https://acp.us"), "{}", blocked.message);
    }
}
