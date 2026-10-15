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

/// Thread page props plus all replies (paged via the InfiniteScroll hook).
pub async fn fetch_thread(http: &reqwest::Client, jar: &Jar, tid: &str) -> Result<(Value, Vec<Value>)> {
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
    if (hook || total > replies.len())
        && let Ok(page) = parse_lv(&final_url, &html)
        && let Ok(mut lv) = Lv::connect(&page, jar).await
    {
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
