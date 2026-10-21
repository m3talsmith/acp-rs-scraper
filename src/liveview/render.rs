//! LiveView rendered-tree merging and HTML rendering (statics + dynamics,
//! comprehensions, components).

use crate::prelude::*;

/// Merge a LiveView diff into the rendered tree (LV rule: a subtree that
/// carries its own statics `s` replaces, otherwise merge key by key).
pub fn lv_merge(base: &mut Value, diff: &Value) {
    match (base.as_object_mut(), diff.as_object()) {
        (Some(b), Some(d)) => {
            for (k, v) in d {
                match b.get_mut(k) {
                    Some(old) if v.is_object() && v.get("s").is_none() && old.is_object() => lv_merge(old, v),
                    _ => { b.insert(k.clone(), v.clone()); }
                }
            }
        }
        _ => *base = diff.clone(),
    }
}

/// Resolve a node's statics: inline, a `p` template table, or another component's.
///
/// LiveView 1.2 puts keyed-comprehension statics in `p` and points `s` at an
/// index. The table may sit on this node or on an ancestor. A negative `s` is
/// another component's statics.
pub fn lv_statics(node: &Value, root: &Value, comps: &Value) -> Vec<String> {
    lv_statics_in(node, root, comps, None)
}

fn lv_statics_in(node: &Value, root: &Value, comps: &Value, inherited: Option<&Value>) -> Vec<String> {
    let strs = |v: &Value| v.as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or_default().to_string()).collect::<Vec<_>>());
    let templates = node.get("p").or(inherited);
    match &node["s"] {
        Value::Array(_) => strs(&node["s"]).unwrap_or_default(),
        Value::Number(n) => {
            let i = n.as_i64().unwrap_or(0);
            if i < 0 {
                lv_statics_in(&comps[(-i).to_string()], root, comps, templates)
            } else if let Some(t) = templates.and_then(|t| t.get(i.to_string())) {
                strs(t).unwrap_or_default()
            } else {
                strs(&root["p"][i.to_string()]).unwrap_or_default()
            }
        }
        _ => vec![],
    }
}

/// Render a rendered-tree node (strings, components, comprehensions) to HTML.
pub fn lv_render(node: &Value, root: &Value, comps: &Value, out: &mut String) {
    lv_render_in(node, root, comps, None, out);
}

fn lv_render_in(node: &Value, root: &Value, comps: &Value, inherited: Option<&Value>, out: &mut String) {
    match node {
        Value::String(s) => out.push_str(s),
        Value::Number(n) => {
            // component reference
            if let Some(c) = comps.get(n.to_string()) { lv_render_in(c, root, comps, inherited, out); }
        }
        Value::Object(_) => {
            let templates = node.get("p").or(inherited);
            let st = lv_statics_in(node, root, comps, inherited);
            if let Some(k) = node.get("k").and_then(Value::as_object) {
                // Keyed comprehension (`k` / `kc`, count inside `k`). The code
                // form can arrive here. `kc` is not an entry. Statics may be a
                // `p` template index rather than an inline list.
                let count = k.get("kc").and_then(Value::as_u64)
                    .or_else(|| node.get("kc").and_then(Value::as_u64))
                    .map(|n| n as usize)
                    .unwrap_or_else(|| {
                        k.keys().filter_map(|key| key.parse::<usize>().ok()).map(|i| i + 1).max().unwrap_or(0)
                    });
                for idx in 0..count {
                    let row = k.get(&idx.to_string());
                    render_parts(&st, |i| row.and_then(|r| r.get(i.to_string())), root, comps, templates, out);
                }
            } else if let Some(rows) = node.get("d").and_then(Value::as_array) {
                for row in rows {
                    render_parts(&st, |i| row.get(i), root, comps, templates, out);
                }
            } else {
                render_parts(&st, |i| node.get(i.to_string()), root, comps, templates, out);
            }
        }
        _ => {}
    }
}

/// Interleave statics with dynamics. The last static has no dynamic after it
/// (LiveView 1.2 `dynamicsToBuffer`).
fn render_parts<'a>(
    statics: &[String], pick: impl Fn(usize) -> Option<&'a Value>, root: &'a Value, comps: &'a Value,
    templates: Option<&'a Value>, out: &mut String,
) {
    if statics.is_empty() { return; }
    for (i, s) in statics.iter().enumerate() {
        out.push_str(s);
        if i + 1 == statics.len() { break; }
        if let Some(v) = pick(i) { lv_render_in(v, root, comps, templates, out); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_comprehension_renders_each_entry() {
        let root = json!({
            "s": ["<li>", "</li>"],
            "k": { "0": {"0": "one"}, "1": {"0": "user[code]"}, "kc": 2 }
        });
        let mut out = String::new();
        lv_render(&root, &root, &json!({}), &mut out);
        assert_eq!(out, "<li>one</li><li>user[code]</li>");
    }

    #[test]
    fn keyed_code_form_reads_statics_from_the_template_table() {
        // Shape of the live `/auth` diff: the form tag is statics in `p`,
        // `s` is an index, and `user[otp]` is dynamic 0 of keyed entry 0.
        let root = json!({
            "s": ["<div>", "</div>"],
            "0": {
                "s": 0,
                "k": { "0": {"0": "<input type=\"hidden\" name=\"_csrf_token\" value=\"form-csrf\"><input name=\"user[otp]\">"}, "kc": 1 },
                "p": {"0": ["<form phx-submit=\"submit-code\" action=\"/auth/user/otp/sign_in\">", "</form>"]}
            }
        });
        let mut out = String::new();
        lv_render(&root, &root, &json!({}), &mut out);
        assert!(out.contains("phx-submit=\"submit-code\""), "{out}");
        assert!(out.contains("name=\"user[otp]\""), "{out}");
        assert!(out.contains("value=\"form-csrf\""), "{out}");
        assert!(out.contains("<div>"), "{out}");
    }
}
