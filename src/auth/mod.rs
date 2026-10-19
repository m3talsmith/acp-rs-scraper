//! Login: email + one-time code over the `/auth` LiveView, finished by the
//! `phx-trigger-action` sign-in POST that sets the session cookie.
//!
//! The socket stays up while the code is read: Phoenix heartbeats go out on
//! [`LV_HEARTBEAT`](crate::liveview::LV_HEARTBEAT). A dropped socket reconnects and the same code is tried
//! again (a fresh join with no code form posts the OTP directly). A wrong code
//! leaves the attempt open.

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

/// Whether the jar's session is accepted by the site: `GET /rs` does not redirect to `/auth`.
pub async fn session_valid(http: &reqwest::Client) -> Result<bool> {
    let resp = http.get(start_url()).send().await?;
    Ok(!resp.url().path().starts_with("/auth"))
}

/// Whether `err` is a dropped LiveView socket (retry / reconnect) rather than a rejected code.
pub fn socket_closed(err: &anyhow::Error) -> bool {
    let s = format!("{err:#}").to_lowercase();
    ["websocket closed", "connection reset", "broken pipe", "connection closed", "already closed"]
        .iter().any(|n| s.contains(n))
}

/// Result of [`LoginSession::start`].
pub enum LoginStart {
    /// `GET /rs` did not redirect to `/auth`. The session file was saved.
    Already,
    /// The email was accepted. The socket is joined and waiting for the code.
    NeedCode(Box<LoginSession>),
}

/// One login attempt. The socket is kept alive while the code is read, and a
/// close reconnects instead of dropping the attempt.
pub struct LoginSession {
    http: reqwest::Client,
    jar: Arc<Jar>,
    email: String,
    page: LvPage,
    lv: Lv,
    email_reply: Value,
    /// `GET /auth` came back already signed in (the socket had died after the cookie was set).
    signed_in: bool,
}

impl LoginSession {
    /// Open `/rs`. When it redirects to `/auth`, submit `email` (or the saved / prompted one).
    pub async fn start(http: &reqwest::Client, jar: &Arc<Jar>, email: Option<String>) -> Result<LoginStart> {
        let resp = http.get(start_url()).send().await?;
        let url = resp.url().to_string();
        let html = resp.text().await?;
        if !url.contains("/auth") {
            info!("Already logged in.");
            save_session(jar)?;
            return Ok(LoginStart::Already);
        }
        let page = parse_lv(&url, &html)?;
        let email = match email {
            Some(e) => { save_email(&e)?; e }
            None => load_email()?,
        };
        let mut lv = Lv::connect(&page, jar).await?;
        let email_reply = submit_email(&mut lv, &email).await?;
        info!("Login email requested for {email}.");
        Ok(LoginStart::NeedCode(Box::new(LoginSession {
            http: http.clone(), jar: jar.clone(), email, page, lv, email_reply, signed_in: false,
        })))
    }

    /// Heartbeat interval for this attempt. `0` is treated as 1ms. Tests set a short interval.
    pub fn set_heartbeat(&mut self, every: std::time::Duration) {
        self.lv.heartbeat = if every.is_zero() { std::time::Duration::from_millis(1) } else { every };
    }

    /// Poll `fut` while heartbeating. A dropped socket reconnects and `fut` keeps running.
    pub async fn pump_until<T>(&mut self, fut: impl std::future::Future<Output = T>) -> Result<T> {
        let mut fut = std::pin::pin!(fut);
        let mut beat = tokio::time::interval(self.lv.heartbeat);
        beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        beat.tick().await;
        let mut drops = 0u8;
        loop {
            enum Ev<T> { Done(T), Beat, Frame(Option<Result<Message, tokio_tungstenite::tungstenite::Error>>) }
            let ev = {
                let ws = &mut self.lv.ws;
                tokio::select! {
                    biased;
                    out = &mut fut => Ev::Done(out),
                    _ = beat.tick() => Ev::Beat,
                    msg = ws.next() => Ev::Frame(msg),
                }
            };
            match ev {
                Ev::Done(out) => return Ok(out),
                Ev::Beat => {
                    if let Err(e) = self.lv.send_heartbeat().await { self.recover(e, &mut drops).await?; } else { drops = 0; }
                }
                Ev::Frame(msg) => {
                    if let Err(e) = self.lv.absorb(msg, None).await { self.recover(e, &mut drops).await?; } else { drops = 0; }
                }
            }
        }
    }

    /// Submit the emailed code (or a login link) and save `~/.acp/session.json`.
    /// A closed socket reconnects and the verify step is tried again, up to three times.
    /// A rejected code is returned as-is and this attempt stays usable.
    pub async fn submit_code(&mut self, code: &str) -> Result<()> {
        let code = code.trim();
        if code.is_empty() { bail!("paste the code (or the login link) from the email"); }
        if self.signed_in || session_valid(&self.http).await? {
            return self.confirm_session().await;
        }
        if code.starts_with("http") {
            self.http.get(code).send().await?.error_for_status()?;
            return self.confirm_session().await;
        }
        let mut last = None;
        for attempt in 0..3 {
            match self.verify_once(code).await {
                Ok(()) => return self.confirm_session().await,
                Err(e) if socket_closed(&e) && attempt + 1 < 3 => {
                    last = Some(e);
                    self.reconnect().await?;
                    if self.signed_in { return self.confirm_session().await; }
                }
                Err(e) => return Err(e),
            }
        }
        Err(last.unwrap_or_else(|| anyhow!("websocket closed")))
    }

    /// Ask for another code on this same attempt. Reconnects first when the socket is gone.
    pub async fn resend(&mut self) -> Result<()> {
        if self.signed_in { return Ok(()); }
        match self.submit_email_here().await {
            Ok(()) => Ok(()),
            Err(e) => {
                if !socket_closed(&e) { verbose!("resend failed ({e:#}); refreshing the login"); }
                self.reconnect().await?;
                if self.signed_in { return Ok(()); }
                self.submit_email_here().await
            }
        }
    }

    async fn submit_email_here(&mut self) -> Result<()> {
        self.email_reply = submit_email(&mut self.lv, &self.email).await?;
        info!("Login email requested again for {}.", self.email);
        Ok(())
    }

    async fn verify_once(&mut self, code: &str) -> Result<()> {
        if !self.ready_for_code() {
            return self.direct_otp_post(code).await;
        }
        let (ev, field, cid) = detect_code_form(&self.lv.html(), &self.email_reply);
        let r2 = self.lv.form(&ev, &field, code, cid).await?;
        if r2["status"] != "ok" { bail!("code submit failed: {r2}"); }
        if let Some(to) = find_redirect(&r2) {
            self.http.get(Url::parse(base_url())?.join(&to)?).send().await?.error_for_status()?;
        } else if self.lv.html().contains("phx-trigger-action") {
            trigger_action_post(&self.http, &self.lv.html(), &self.page.csrf, &self.email, &field, code).await?;
        } else {
            self.direct_otp_post(code).await?;
        }
        Ok(())
    }

    /// The fresh join after a reconnect only has the email form. Post the OTP without that form.
    async fn direct_otp_post(&self, code: &str) -> Result<()> {
        let fields: Vec<(String, String)> = vec![
            ("_csrf_token".into(), self.page.csrf.clone()),
            (EMAIL_FIELD.into(), self.email.clone()),
            (CODE_FIELD_FALLBACK.into(), code.into()),
        ];
        verbose!("POST {} directly (the LiveView form was gone)", OTP_SIGN_IN_FALLBACK);
        let resp = self.http.post(Url::parse(base_url())?.join(OTP_SIGN_IN_FALLBACK)?)
            .header("Origin", base_url())
            .header("Referer", format!("{}/auth", base_url()))
            .header("Sec-Fetch-Mode", "navigate")
            .form(&fields)
            .send().await?;
        if !resp.status().is_success() { bail!("sign-in POST failed: {}", resp.status()); }
        Ok(())
    }

    fn ready_for_code(&self) -> bool {
        let html = self.lv.html();
        html.contains(CODE_FIELD_FALLBACK) || html.contains("phx-trigger-action")
            || html.contains(&format!("phx-submit=\"{CODE_EVENT_FALLBACK}\""))
    }

    async fn confirm_session(&mut self) -> Result<()> {
        if !session_valid(&self.http).await? { bail!("still not logged in after verification"); }
        info!("Logged in.");
        save_session(&self.jar)?;
        self.signed_in = true;
        Ok(())
    }

    async fn recover(&mut self, e: anyhow::Error, drops: &mut u8) -> Result<()> {
        if !socket_closed(&e) { return Err(e); }
        *drops += 1;
        if *drops > 3 { return Err(e); }
        verbose!("login socket closed; reconnecting");
        self.reconnect().await
    }

    async fn reconnect(&mut self) -> Result<()> {
        let every = self.lv.heartbeat;
        let resp = self.http.get(format!("{}/auth", base_url())).send().await.context("reconnecting the login")?;
        let url = resp.url().to_string();
        let html = resp.text().await?;
        if !url.contains("/auth") {
            self.signed_in = true;
            return Ok(());
        }
        self.page = parse_lv(&url, &html)?;
        self.lv = Lv::connect(&self.page, &self.jar).await?;
        self.lv.heartbeat = every;
        Ok(())
    }
}

async fn submit_email(lv: &mut Lv, email: &str) -> Result<Value> {
    let r = lv.form(EMAIL_EVENT, EMAIL_FIELD, email, Some(EMAIL_FORM_CID)).await?;
    if r["status"] != "ok" { bail!("email submit failed: {r}"); }
    Ok(r)
}

/// Log in: open `/rs` (redirects to `/auth` when logged out), submit the saved email over the LiveView, prompt for the one-time code (or login link), submit it and finish with the sign-in POST. Fails if `/rs` still redirects to `/auth`. A saved session in the jar (see [`session`]) skips the code; the session is saved again on success.
///
/// The prompt runs on a blocking thread so this task can heartbeat the LiveView socket while the code is typed.
pub async fn login(http: &reqwest::Client, jar: &Arc<Jar>) -> Result<()> {
    login_flow(http, jar, None, || async {
        tokio::task::spawn_blocking(|| prompt("Verification code (or paste the login link): "))
            .await
            .context("reading the verification code")?
    }).await
}

/// The login steps behind [`login`], with the email and the code supplied by the caller
/// (the terminal UI's Login modal passes the typed email and awaits the typed code).
/// `email = None` reads it from `~/.acp/preferences.toml` (prompting when missing);
/// `Some` is used as given and saved there. `code` is only called when the site asks.
/// Heartbeats run while `code` is pending. A dropped socket reconnects before the code is submitted.
pub async fn login_flow<F, Fut>(http: &reqwest::Client, jar: &Arc<Jar>, email: Option<String>, code: F) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<String>>,
{
    let LoginStart::NeedCode(session) = LoginSession::start(http, jar, email).await? else { return Ok(()) };
    let mut session = *session;
    let code = session.pump_until(code()).await??;
    session.submit_code(&code).await
}

#[cfg(test)]
mod tests {
    use super::socket_closed;

    #[test]
    fn login_socket_errors_are_retries_and_a_bad_code_is_not() {
        assert!(socket_closed(&anyhow::anyhow!("websocket closed")));
        assert!(socket_closed(&anyhow::anyhow!("websocket closed: connection reset by peer")));
        assert!(socket_closed(&anyhow::anyhow!("broken pipe")));
        assert!(!socket_closed(&anyhow::anyhow!("sign-in POST failed: 403 Forbidden")));
        assert!(!socket_closed(&anyhow::anyhow!("still not logged in after verification")));
    }
}
