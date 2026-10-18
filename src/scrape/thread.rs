//! Thread pages and their replies (InfiniteScroll paging).

use crate::prelude::*;

/// Collect every object having an `id` and `inserted_at` from any `replies`
/// array in a JSON value, including JSON hidden in strings (diffs / data-props).
pub fn harvest_replies(v: &Value, out: &mut Vec<Value>) {
    match v {
        Value::Object(m) => {
            if let Some(Value::Array(rs)) = m.get("replies") {
                out.extend(rs.iter().filter(|r| r.get("inserted_at").is_some()).cloned());
            }
            m.values().for_each(|x| harvest_replies(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| harvest_replies(x, out)),
        Value::String(st) if st.contains("replies") => {
            let st = st.replace("&quot;", "\"").replace("&amp;", "&");
            if let Some(i) = st.find('{')
                && let Ok(j) = serde_json::from_str::<Value>(&st[i..st.rfind('}').unwrap_or(i) + 1]) {
                    harvest_replies(&j, out);
                }
        }
        _ => {}
    }
}

/// How a thread's replies are ordered. Newest is the site's default (no hook).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThreadSort {
    /// Newest first (`posted_at` descending). The page as it opens.
    #[default]
    Newest,
    /// Oldest first. Site hook `sort {sort:"oldest"}`.
    Oldest,
    /// Highest score first. Site hook `sort {sort:"top"}`.
    Top,
}

impl ThreadSort {
    /// Next order in the `s` cycle: newest → oldest → top.
    pub fn cycle(self) -> Self {
        match self { ThreadSort::Newest => ThreadSort::Oldest, ThreadSort::Oldest => ThreadSort::Top, ThreadSort::Top => ThreadSort::Newest }
    }
    /// Status-line name.
    pub fn label(self) -> &'static str {
        match self { ThreadSort::Newest => "newest", ThreadSort::Oldest => "oldest", ThreadSort::Top => "top" }
    }
    /// The hook value, when this order is not the site default.
    pub fn hook(self) -> Option<&'static str> {
        match self { ThreadSort::Newest => None, ThreadSort::Oldest => Some("oldest"), ThreadSort::Top => Some("top") }
    }
    /// Parse `oldest` or `top`.
    pub fn parse_hook(s: &str) -> std::result::Result<ThreadSort, String> {
        match s.trim() {
            "oldest" => Ok(ThreadSort::Oldest),
            "top" => Ok(ThreadSort::Top),
            other => Err(format!("thread sort must be oldest or top, not {other:?}")),
        }
    }
}

/// Reorder `replies` in place. Top puts a missing score last.
pub fn sort_replies(replies: &mut [Post], sort: ThreadSort) {
    match sort {
        ThreadSort::Newest => replies.sort_by(|a, b| b.posted_at.cmp(&a.posted_at).then(a.post_id.cmp(&b.post_id))),
        ThreadSort::Oldest => replies.sort_by(|a, b| a.posted_at.cmp(&b.posted_at).then(a.post_id.cmp(&b.post_id))),
        ThreadSort::Top => replies.sort_by(|a, b| b.score.cmp(&a.score).then(b.posted_at.cmp(&a.posted_at)).then(a.post_id.cmp(&b.post_id))),
    }
}

/// Thread page props plus all replies (paged via the InfiniteScroll hook).
pub async fn fetch_thread(http: &reqwest::Client, jar: &Jar, tid: &str) -> Result<(Value, Vec<Value>)> {
    fetch_thread_sorted(http, jar, tid, None).await
}

/// [`fetch_thread`], then the `sort` hook when `sort` is `oldest` or `top`.
/// Paging stays on that LiveView so the order is not reset by a second connection.
pub async fn fetch_thread_sorted(http: &reqwest::Client, jar: &Jar, tid: &str, sort: Option<&str>) -> Result<(Value, Vec<Value>)> {
    let url = thread_url(tid);
    let resp = http.get(&url).send().await?;
    let final_url = resp.url().to_string();
    let html = resp.text().await?;
    let th = props(&html, THREAD_PROPS_SEL).with_context(|| format!("no forum/Thread props for {tid}"))?;
    let mut replies: Vec<Value> = th["replies"].as_array().cloned().unwrap_or_default();
    // Thread.js has no load-more event; more replies come from the
    // InfiniteScroll hook (`pushEventTo(this.el, "next-page", {})`),
    // whose target cid is the element's phx-target (none = root view).
    // New replies arrive as a diff re-rendering forum/Thread data-props.
    let (hook, cid) = {
        let doc = Html::parse_document(&html);
        let sel = Selector::parse("[phx-hook=InfiniteScroll],[data-phx-hook=InfiniteScroll]").unwrap();
        let el = doc.select(&sel).next();
        (el.is_some(), el.and_then(|e| e.value().attr("phx-target").and_then(|t| t.parse::<u64>().ok())))
    };
    let total = th.pointer("/root/replies_count").and_then(Value::as_u64).unwrap_or(0) as usize;
    let paging = hook || total > replies.len();
    if sort.is_none() && !paging { return Ok((th, replies)); }
    let page = match parse_lv(&final_url, &html) {
        Ok(p) => p,
        Err(e) if sort.is_some() => return Err(e),
        Err(_) => return Ok((th, replies)),
    };
    let mut lv = match Lv::connect(&page, jar).await {
        Ok(lv) => lv,
        Err(e) if sort.is_some() => return Err(e),
        Err(_) => return Ok((th, replies)),
    };
    if let Some(order) = sort {
        let ev = json!({"type": "hook", "event": THREAD_SORT_EVENT, "value": {"sort": order}});
        let r = lv.push("event", ev).await.with_context(|| format!("sort {order} on {tid}"))?;
        if r["status"] != "ok" { bail!("sort {order} failed: {r}"); }
        if let Some(p) = props(&lv.html(), THREAD_PROPS_SEL) {
            if let Some(rs) = p["replies"].as_array() { replies = rs.clone(); }
        }
        let mut got = Vec::new();
        harvest_replies(reply_body(&r), &mut got);
        if replies.is_empty() && !got.is_empty() { replies = got; }
    }
    if paging {
        let mut ids: HashSet<String> = replies.iter().map(|x| s(x, "id")).collect();
        for _ in 0..MAX_PAGES {
            let mut ev = json!({"type": "hook", "event": THREAD_NEXT_EVENT, "value": {}});
            if let Some(c) = cid { ev["cid"] = json!(c); }
            let Ok(r) = lv.push("event", ev).await else { break };
            let mut got = Vec::new();
            harvest_replies(reply_body(&r), &mut got);
            if let Some(p) = props(&lv.html(), THREAD_PROPS_SEL) { harvest_replies(&p, &mut got); }
            let new: Vec<Value> = got.into_iter().filter(|x| ids.insert(s(x, "id"))).collect();
            if new.is_empty() { break; }
            replies.extend(new);
        }
    }
    Ok((th, replies))
}

/// `--sort-thread`: read the thread in `oldest` or `top` order and store scores.
/// Text already stored is not replaced.
pub async fn read_sorted_thread(http: &reqwest::Client, jar: &Jar, thread_id: &str, order: &str) -> Result<usize> {
    let sort = ThreadSort::parse_hook(order).map_err(|e| anyhow!(e))?;
    let hook = sort.hook().unwrap_or("oldest");
    let (th, replies) = fetch_thread_sorted(http, jar, thread_id, Some(hook)).await?;
    let mut posts = Vec::new();
    let mut seen = HashSet::new();
    if let Some(root) = th.get("root") {
        let p = to_post(root, "", "");
        if !p.post_id.is_empty() && seen.insert(p.post_id.clone()) { posts.push(p); }
    }
    for r in &replies {
        let p = to_post(r, thread_id, "");
        if p.post_id.is_empty() || p.post_id == thread_id || !seen.insert(p.post_id.clone()) { continue; }
        posts.push(p);
    }
    save(&posts)?;
    info!("thread {thread_id} sorted {hook}: {} posts", posts.len());
    Ok(posts.len())
}
