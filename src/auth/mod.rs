//! Login: email + one-time code over the `/auth` LiveView, finished by the
//! `phx-trigger-action` sign-in POST that sets the session cookie.

pub mod prefs;
pub mod session;

use crate::prelude::*;

/// Look for a redirect target anywhere in a LiveView reply.
pub fn find_redirect(v: &Value) -> Option<String> {
    for k in ["redirect", "live_redirect", "navigate"] {
        if let Some(to) = v.pointer(&format!("/response/{k}/to")).and_then(Value::as_str) {
            return Some(to.into());
        }
    }
    None
}

/// Discover the code form's event / field / cid from the rendered LiveView
/// (falls back to the raw reply text, then to constants).
pub fn detect_code_form(html: &str, reply: &Value) -> (String, String, Option<u64>) {
    let raw = format!("{html}{}", reply.to_string().replace("\\\"", "\""));
    let ev = regex::Regex::new(r#"phx-submit="([^"]+)""#).unwrap()
        .captures_iter(&raw).map(|c| c[1].to_string())
        .find(|e| e != EMAIL_EVENT)
        .unwrap_or_else(|| CODE_EVENT_FALLBACK.into());
    let doc = Html::parse_fragment(html);
    let field = doc.select(&Selector::parse("form input").unwrap())
        .filter(|i| !matches!(i.value().attr("type"), Some("hidden" | "email")))
        .filter_map(|i| i.value().attr("name").map(str::to_string))
        .find(|n| n != EMAIL_FIELD && !n.starts_with('_'))
        .unwrap_or_else(|| CODE_FIELD_FALLBACK.into());
    let cid = Selector::parse(&format!("form[phx-submit='{ev}']")).ok()
        .and_then(|sel| doc.select(&sel).next()
            .and_then(|f| f.value().attr("phx-target")?.parse().ok()))
        .or(Some(EMAIL_FORM_CID));
    (ev, field, cid)
}

/// `phx-trigger-action`: LiveView asks the client to do a real HTTP POST of
/// the form (action="/auth/user/otp/sign_in") so the server can set the
/// authenticated session cookie.
pub async fn trigger_action_post(
    http: &reqwest::Client, html: &str, csrf: &str, email: &str, code_field: &str, code: &str,
) -> Result<()> {
    let doc = Html::parse_fragment(html);
    let form = doc.select(&Selector::parse("form[phx-trigger-action]").unwrap()).next()
        .ok_or_else(|| anyhow!("no redirect and no phx-trigger-action form after code"))?;
    let action = form.value().attr("action").unwrap_or(OTP_SIGN_IN_FALLBACK);
    let mut fields: Vec<(String, String)> = Vec::new();
    for i in form.select(&Selector::parse("input[name]").unwrap()) {
        let name = i.value().attr("name").unwrap_or_default().to_string();
        let mut val = i.value().attr("value").unwrap_or_default().to_string();
        if val.is_empty() {
            if name == EMAIL_FIELD { val = email.into(); }
            else if name == code_field { val = code.into(); }
        }
        fields.push((name, val));
    }
    if !fields.iter().any(|(n, _)| n == "_csrf_token") {
        fields.push(("_csrf_token".into(), csrf.into()));
    }
    let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
    verbose!("POST {action} with fields {names:?}");
    let resp = http.post(Url::parse(base_url())?.join(action)?)
        .header("Origin", base_url())
        .header("Referer", format!("{}/auth", base_url()))
        .header("Sec-Fetch-Mode", "navigate")
        .form(&fields)
        .send().await?;
    if !resp.status().is_success() {
        bail!("sign-in POST failed: {}", resp.status());
    }
    Ok(())
}

/// Log in: open `/rs` (redirects to `/auth` when logged out), submit the saved email over the LiveView, prompt for the one-time code (or login link), submit it and finish with the sign-in POST. Fails if `/rs` still redirects to `/auth`. A saved session in the jar (see [`session`]) skips the code; the session is saved again on success.
pub async fn login(http: &reqwest::Client, jar: &Arc<Jar>) -> Result<()> {
    let resp = http.get(start_url()).send().await?;
    let url = resp.url().to_string();
    let html = resp.text().await?;
    if !url.contains("/auth") {
        info!("Already logged in.");
        save_session(jar)?;
        return Ok(());
    }
    let page = parse_lv(&url, &html)?;
    let email = load_email()?;
    let mut lv = Lv::connect(&page, jar).await?;
    let r = lv.form(EMAIL_EVENT, EMAIL_FIELD, &email, Some(EMAIL_FORM_CID)).await?;
    if r["status"] != "ok" {
        bail!("email submit failed: {r}");
    }
    info!("Login email requested for {email}.");
    let code = prompt("Verification code (or paste the login link): ")?;
    if code.starts_with("http") {
        http.get(&code).send().await?.error_for_status()?;
    } else {
        let (ev, field, cid) = detect_code_form(&lv.html(), &r);
        let r2 = lv.form(&ev, &field, &code, cid).await?;
        if let Some(to) = find_redirect(&r2) {
            http.get(Url::parse(base_url())?.join(&to)?).send().await?.error_for_status()?;
        } else {
            trigger_action_post(http, &lv.html(), &page.csrf, &email, &field, &code).await?;
        }
    }
    let check = http.get(start_url()).send().await?;
    if check.url().path().starts_with("/auth") {
        bail!("still not logged in after verification");
    }
    info!("Logged in.");
    save_session(jar)?;
    Ok(())
}
