//! Phoenix LiveView client: page bootstrap parsing, websocket join/push,
//! and helpers to read hook replies and Svelte `data-props`.

pub mod render;

use crate::prelude::*;

/// LiveView bootstrap data read from a page's HTML.
pub struct LvPage {
    /// Page / media URL.
    pub url: String,
    /// CSRF token (`meta[name=csrf-token]`).
    pub csrf: String,
    /// Root LiveView element id.
    pub id: String,
    /// Signed `data-phx-session`.
    pub session: String,
    /// Signed `data-phx-static`.
    pub statik: String,
}

/// Read the LiveView bootstrap (CSRF token, root id, session, static) from a page.
pub fn parse_lv(url: &str, html: &str) -> Result<LvPage> {
    let doc = Html::parse_document(html);
    let csrf = doc
        .select(&Selector::parse("meta[name=csrf-token]").unwrap())
        .next()
        .and_then(|m| m.value().attr("content"))
        .context("no csrf-token")?;
    let main = doc
        .select(&Selector::parse("[data-phx-main]").unwrap())
        .next()
        .context("no LiveView root")?
        .value();
    Ok(LvPage {
        url: url.into(),
        csrf: csrf.into(),
        id: main.attr("id").unwrap_or_default().into(),
        session: main.attr("data-phx-session").unwrap_or_default().into(),
        statik: main.attr("data-phx-static").unwrap_or_default().into(),
    })
}

/// Remove reply/events/title keys before merging a diff into the tree.
pub fn strip_meta(d: &Value) -> Value {
    let mut d = d.clone();
    if let Some(m) = d.as_object_mut() { for k in ["r", "e", "t"] { m.remove(k); } }
    d
}

/// The LiveView websocket stream type.
pub type Ws = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// A joined LiveView: websocket, channel topic, message ref counter and the merged rendered tree.
pub struct Lv {
    /// Websocket stream.
    pub ws: Ws,
    /// Channel topic (`lv:<id>`).
    pub topic: String,
    /// Last message ref.
    pub r: u64,
    /// Merged rendered tree.
    pub rendered: Value,
}

impl Lv {
    /// Open the LiveView websocket with the session cookies and join the page's root view.
    pub async fn connect(p: &LvPage, jar: &Jar) -> Result<Self> {
        let u = ws_url(&urlenc(&p.csrf));
        let mut req = u.into_client_request()?;
        let h = req.headers_mut();
        h.insert("User-Agent", FIREFOX_UA.parse()?);
        h.insert("Origin", base_url().parse()?);
        if let Some(c) = jar.cookies(&Url::parse(base_url())?) {
            h.insert("Cookie", c);
        }
        let (ws, _) = tokio_tungstenite::connect_async(req).await?;
        let mut lv = Lv { ws, topic: format!("lv:{}", p.id), r: 0, rendered: json!({}) };
        let reply = lv
            .push("phx_join", json!({
                "url": p.url,
                "params": {"_csrf_token": p.csrf, "_mounts": 0},
                "session": p.session,
                "static": p.statik,
            }))
            .await?;
        if reply["status"] != "ok" {
            bail!("LiveView join failed: {reply}");
        }
        lv.rendered = reply["response"]["rendered"].clone();
        Ok(lv)
    }

    /// Push an event and wait for its reply, merging every diff seen into [`Lv::rendered`].
    pub async fn push(&mut self, event: &str, payload: Value) -> Result<Value> {
        self.r += 1;
        let r = self.r.to_string();
        let msg = json!(["1", r, self.topic, event, payload]);
        dbg_log("push", &msg.to_string());
        self.ws.send(Message::text(msg.to_string())).await?;
        while let Some(m) = self.ws.next().await {
            if let Message::Text(t) = m? {
                let v: Value = serde_json::from_str(&t)?;
                dbg_log("recv", &t);
                if v[3] == "diff" {
                    lv_merge(&mut self.rendered, &strip_meta(&v[4]));
                }
                if v[1] == r.as_str() && v[3] == "phx_reply" {
                    if let Some(d) = v[4].pointer("/response/diff") {
                        lv_merge(&mut self.rendered, &strip_meta(d));
                    }
                    return Ok(v[4].clone());
                }
            }
        }
        bail!("websocket closed")
    }

    /// Render the current LiveView tree (root plus every component) to HTML.
    pub fn html(&self) -> String {
        let mut out = String::new();
        let comps = self.rendered.get("c").cloned().unwrap_or(json!({}));
        lv_render(&self.rendered, &self.rendered, &comps, &mut out);
        if let Some(m) = comps.as_object() {
            for c in m.values() { lv_render(c, &self.rendered, &comps, &mut out); }
        }
        out
    }

    /// Submit a LiveView form event (`field=val`, URL-encoded), optionally to component `cid`.
    pub async fn form(&mut self, event: &str, field: &str, val: &str, cid: Option<u64>) -> Result<Value> {
        let mut p = json!({
            "type": "form",
            "event": event,
            "value": format!("{}={}", urlenc(field), urlenc(val)),
        });
        if let Some(c) = cid {
            p["cid"] = json!(c);
        }
        self.push("event", p).await
    }
}

/// `application/x-www-form-urlencoded` encoding of `s`.
pub fn urlenc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Parse the JSON `data-props` of the first element matching CSS `sel` (a Svelte mount point).
pub fn props(html: &str, sel: &str) -> Option<Value> {
    let doc = Html::parse_document(html);
    let el = doc.select(&Selector::parse(sel).ok()?).next()?;
    serde_json::from_str(el.value().attr("data-props")?).ok()
}

/// LiveView put `{:reply, map}` under response.r (LV >= 0.18) or response itself.
/// Verified in bundled phoenix_live_view 1.2.12: a hook pushEvent reply is
/// `phx_reply {status:"ok", response:{diff:{..., "r": <reply>}}}`
/// (Rendered.extract pops keys "r" reply, "e" events, "t" title from the diff).
pub fn reply_body(v: &Value) -> &Value {
    let r = &v["response"];
    if let Some(x) = r.pointer("/diff/r") { x } else if let Some(x) = r.get("r") { x } else { r }
}
