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

/// How often the client sends a Phoenix heartbeat. The server closes a socket that stays quiet for about a minute.
pub const LV_HEARTBEAT: std::time::Duration = std::time::Duration::from_secs(30);

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
    /// How often to send a Phoenix heartbeat while waiting on this socket.
    pub heartbeat: std::time::Duration,
    /// Ref of a heartbeat that has not been answered. The next heartbeat
    /// while this is set means the socket went quiet and is dead.
    pub hb_pending: Option<String>,
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
        let mut lv = Lv { ws, topic: format!("lv:{}", p.id), r: 0, rendered: json!({}), heartbeat: LV_HEARTBEAT, hb_pending: None };
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

    /// Leave the joined page and join `to` with a LiveView `redirect`.
    ///
    /// The reply edit capture opens the editor this way. The new render replaces
    /// [`Lv::rendered`]. `_track_static` is omitted, the same way the compose join omits it.
    pub async fn live_redirect(&mut self, page: &LvPage, to: &str) -> Result<()> {
        self.push("phx_leave", json!({})).await.context("leaving the page before the edit")?;
        let reply = self
            .push(
                "phx_join",
                json!({
                    "redirect": to,
                    "params": {"_csrf_token": page.csrf, "_mounts": 0, "_live_referer": page.url},
                    "session": page.session,
                    "static": page.statik,
                    "sticky": false,
                }),
            )
            .await
            .context("opening the edit page")?;
        if reply["status"] != "ok" {
            bail!("the edit page did not open: {}", crate::logging::redact_secrets(&reply.to_string()));
        }
        if let Some(rendered) = reply.pointer("/response/rendered") {
            self.rendered = rendered.clone();
        }
        Ok(())
    }

    /// Push an event and wait for its reply, merging every diff seen into [`Lv::rendered`].
    ///
    /// Phoenix heartbeats go out on [`LV_HEARTBEAT`] while the reply is outstanding.
    /// A heartbeat with no `phx_reply` before the next one means the socket is dead.
    /// A protocol ping is answered by flushing the queued pong.
    pub async fn push(&mut self, event: &str, payload: Value) -> Result<Value> {
        self.r += 1;
        let want = self.r.to_string();
        let msg = json!(["1", &want, self.topic, event, payload]);
        dbg_log("push", &msg.to_string());
        self.send_text(&msg).await?;
        self.wait_reply(&want, None).await
    }

    /// Join a topic whose `join_ref` is the same as the message ref.
    ///
    /// The upload channel does this (`lvu:<entry ref>`). The page channel keeps
    /// join ref `"1"`. The returned string is the join ref to reuse on later
    /// pushes to this topic.
    pub async fn join_topic(&mut self, topic: &str, payload: Value) -> Result<(String, Value)> {
        self.r += 1;
        let want = self.r.to_string();
        let msg = json!([&want, &want, topic, "phx_join", payload]);
        dbg_log("push", &msg.to_string());
        self.send_text(&msg).await?;
        let reply = self.wait_reply(&want, None).await?;
        Ok((want, reply))
    }

    /// Push a Phoenix V2 binary frame and wait for the text `phx_reply` on `msg_ref`.
    ///
    /// The debug line names the topic and the byte count. The file bytes are not logged.
    pub async fn push_binary(
        &mut self,
        join_ref: &str,
        topic: &str,
        event: &str,
        bytes: &[u8],
        timeout: std::time::Duration,
    ) -> Result<Value> {
        self.r += 1;
        let want = self.r.to_string();
        let bin = encode_binary_push(join_ref, &want, topic, event, bytes)?;
        dbg_log("push", &format!("binary {topic} chunk {} bytes", bytes.len()));
        self.ws
            .send(Message::binary(bin))
            .await
            .map_err(|e| anyhow!("websocket closed: {e}"))?;
        self.wait_reply(&want, Some(timeout)).await
    }

    /// Leave a topic. A missing reply, or a socket that is already closed, is ignored.
    pub async fn leave_topic(&mut self, join_ref: &str, topic: &str) {
        self.r += 1;
        let want = self.r.to_string();
        let msg = json!([join_ref, &want, topic, "phx_leave", {}]);
        dbg_log("push", &msg.to_string());
        if self.send_text(&msg).await.is_err() {
            return;
        }
        let _ = self.wait_reply(&want, Some(std::time::Duration::from_secs(2))).await;
    }

    /// Wait until `phx_reply` for `want`. `timeout` bounds an upload chunk. A page push passes `None`.
    async fn wait_reply(&mut self, want: &str, timeout: Option<std::time::Duration>) -> Result<Value> {
        let started = tokio::time::Instant::now();
        let mut beat = tokio::time::interval(self.heartbeat);
        beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        beat.tick().await;
        loop {
            let timeout_sleep = async {
                match timeout {
                    Some(limit) => tokio::time::sleep(limit.saturating_sub(started.elapsed())).await,
                    None => std::future::pending::<()>().await,
                }
            };
            let frame = {
                let ws = &mut self.ws;
                tokio::select! {
                    biased;
                    msg = ws.next() => Some(msg),
                    _ = beat.tick() => None,
                    _ = timeout_sleep => {
                        bail!("upload timed out waiting for a reply");
                    }
                }
            };
            match frame {
                None => self.send_heartbeat().await?,
                Some(msg) => {
                    if let Some(reply) = self.absorb(msg, Some(want)).await? {
                        return Ok(reply);
                    }
                }
            }
        }
    }

    /// Send one Phoenix heartbeat (`[null, ref, "phoenix", "heartbeat", {}]`).
    ///
    /// Fails with `websocket closed` when the previous heartbeat is still
    /// unanswered. A write that succeeds is not proof the socket is alive.
    pub async fn send_heartbeat(&mut self) -> Result<()> {
        if self.hb_pending.is_some() {
            bail!("websocket closed: heartbeat unanswered");
        }
        self.r += 1;
        let r = self.r.to_string();
        let msg = json!([Value::Null, r.clone(), "phoenix", "heartbeat", {}]);
        dbg_log("push", &msg.to_string());
        self.send_text(&msg).await?;
        self.hb_pending = Some(r);
        Ok(())
    }

    async fn send_text(&mut self, msg: &Value) -> Result<()> {
        self.ws.send(Message::text(msg.to_string())).await.map_err(|e| anyhow!("websocket closed: {e}"))
    }

    /// Apply one incoming frame. `want` is the ref of a `phx_reply` to return.
    /// `Ok(None)` means the wait continues. A closed socket is `websocket closed`.
    pub(crate) async fn absorb(&mut self, msg: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>, want: Option<&str>) -> Result<Option<Value>> {
        let msg = match msg {
            None => bail!("websocket closed"),
            Some(Err(e)) => bail!("websocket closed: {e}"),
            Some(Ok(m)) => m,
        };
        match msg {
            Message::Ping(_) => {
                // `read` already queued the matching pong. Flush that one.
                // A second pong replaces the queued reply and can drop it
                // (tungstenite: do not answer a ping by hand).
                self.ws.flush().await.map_err(|e| anyhow!("websocket closed: {e}"))?;
                Ok(None)
            }
            Message::Close(_) => bail!("websocket closed"),
            Message::Text(t) => {
                let text = t.as_str();
                let v: Value = serde_json::from_str(text)?;
                dbg_log("recv", text);
                if v[3] == "phx_reply" && self.hb_pending.as_deref() == json_ref(&v[1]).as_deref() {
                    self.hb_pending = None;
                }
                if v[3] == "diff" { lv_merge(&mut self.rendered, &strip_meta(&v[4])); }
                if let Some(want) = want
                    && v[1] == want && v[3] == "phx_reply" {
                        if let Some(d) = v[4].pointer("/response/diff") { lv_merge(&mut self.rendered, &strip_meta(d)); }
                        return Ok(Some(v[4].clone()));
                    }
                Ok(None)
            }
            Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => Ok(None),
        }
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

    /// Read frames until `limit` elapses, the socket closes, or the buffer ends.
    ///
    /// A closed socket returns the frames already read. This does not send.
    pub async fn collect_for(&mut self, limit: std::time::Duration) -> Result<Vec<Value>> {
        let deadline = tokio::time::Instant::now() + limit;
        let mut frames = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                break;
            }
            let msg = match tokio::time::timeout(left, self.ws.next()).await {
                Err(_) => break,
                Ok(m) => m,
            };
            let text = match &msg {
                Some(Ok(Message::Text(t))) => Some(t.to_string()),
                _ => None,
            };
            match self.absorb(msg, None).await {
                Ok(_) => {}
                Err(e) if e.to_string().contains("websocket closed") => break,
                Err(e) => return Err(e),
            }
            if let Some(text) = text
                && let Ok(v) = serde_json::from_str::<Value>(&text)
            {
                frames.push(v);
            }
        }
        Ok(frames)
    }

    /// Send a close frame and flush it. Errors from an already-dead socket are ignored
    /// by the caller; this only tries.
    pub async fn close(&mut self) {
        let _ = self.ws.close(None).await;
    }

    /// Submit a LiveView form event (`field=val`, URL-encoded), optionally to component `cid`.
    pub async fn form(&mut self, event: &str, field: &str, val: &str, cid: Option<u64>) -> Result<Value> {
        self.form_fields(event, &[(field, val)], cid).await
    }

    /// Submit a LiveView form event with several fields (`k=v&…`, URL-encoded), optionally to component `cid`.
    pub async fn form_fields(&mut self, event: &str, fields: &[(&str, &str)], cid: Option<u64>) -> Result<Value> {
        let mut p = json!({
            "type": "form",
            "event": event,
            "value": form_query(fields),
        });
        if let Some(c) = cid {
            p["cid"] = json!(c);
        }
        self.push("event", p).await
    }
}

/// Phoenix refs are JSON strings. A numeric ref is accepted too.
fn json_ref(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
        .or_else(|| v.as_i64().map(|n| n.to_string()))
        .or_else(|| v.as_u64().map(|n| n.to_string()))
}

/// `application/x-www-form-urlencoded` encoding of `s`.
pub fn urlenc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// URL-encoded `k=v&…` body of a LiveView form event.
pub fn form_query(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(k, v)| format!("{}={}", urlenc(k), urlenc(v))).collect::<Vec<_>>().join("&")
}

/// Parse the JSON `data-props` of the first element matching CSS `sel` (a Svelte mount point).
pub fn props(html: &str, sel: &str) -> Option<Value> {
    let doc = Html::parse_document(html);
    let el = doc.select(&Selector::parse(sel).ok()?).next()?;
    serde_json::from_str(el.value().attr("data-props")?).ok()
}

/// Phoenix V2 binary push. `kind` 0, then one length byte each for join ref, ref, topic, and event, then those strings, then the raw bytes.
///
/// A captured chunk header is 21 bytes when the refs are three digits, the topic is `lvu:1`, and the event is `chunk`: 5 length bytes + 3 + 3 + 5 + 5.
pub fn encode_binary_push(join_ref: &str, msg_ref: &str, topic: &str, event: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    let jr = join_ref.as_bytes();
    let rf = msg_ref.as_bytes();
    let tp = topic.as_bytes();
    let ev = event.as_bytes();
    if jr.len() > 255 || rf.len() > 255 || tp.len() > 255 || ev.len() > 255 {
        bail!("a binary push field is longer than 255 bytes");
    }
    let mut out = Vec::with_capacity(5 + jr.len() + rf.len() + tp.len() + ev.len() + bytes.len());
    out.push(0);
    out.push(jr.len() as u8);
    out.push(rf.len() as u8);
    out.push(tp.len() as u8);
    out.push(ev.len() as u8);
    out.extend_from_slice(jr);
    out.extend_from_slice(rf);
    out.extend_from_slice(tp);
    out.extend_from_slice(ev);
    out.extend_from_slice(bytes);
    Ok(out)
}

/// Kind, join ref, message ref, topic, event, and the trailing bytes of a Phoenix V2 binary push.
pub type BinaryPush<'a> = (u8, String, String, String, String, &'a [u8]);

/// Split a Phoenix V2 binary push into kind, join ref, ref, topic, event, and the trailing bytes.
pub fn decode_binary_push(bytes: &[u8]) -> Option<BinaryPush<'_>> {
    if bytes.len() < 5 {
        return None;
    }
    let kind = bytes[0];
    let jl = bytes[1] as usize;
    let rl = bytes[2] as usize;
    let tl = bytes[3] as usize;
    let el = bytes[4] as usize;
    let mut i = 5usize;
    let take = |i: &mut usize, n: usize| -> Option<String> {
        let end = i.checked_add(n)?;
        let slice = bytes.get(*i..end)?;
        *i = end;
        String::from_utf8(slice.to_vec()).ok()
    };
    let join_ref = take(&mut i, jl)?;
    let msg_ref = take(&mut i, rl)?;
    let topic = take(&mut i, tl)?;
    let event = take(&mut i, el)?;
    Some((kind, join_ref, msg_ref, topic, event, bytes.get(i..)?))
}

/// LiveView put `{:reply, map}` under response.r (LV >= 0.18) or response itself.
/// Verified in bundled phoenix_live_view 1.2.12: a hook pushEvent reply is
/// `phx_reply {status:"ok", response:{diff:{..., "r": <reply>}}}`
/// (Rendered.extract pops keys "r" reply, "e" events, "t" title from the diff).
pub fn reply_body(v: &Value) -> &Value {
    let r = &v["response"];
    if let Some(x) = r.pointer("/diff/r") { x } else if let Some(x) = r.get("r") { x } else { r }
}
