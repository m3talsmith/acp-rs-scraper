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

/// Resolve a node's statics: inline, shared (`p` table), or another component's.
pub fn lv_statics<'a>(node: &'a Value, root: &'a Value, comps: &'a Value) -> Vec<String> {
    let strs = |v: &Value| v.as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or_default().to_string()).collect::<Vec<_>>());
    match &node["s"] {
        Value::Array(_) => strs(&node["s"]).unwrap_or_default(),
        Value::Number(n) => {
            let i = n.as_i64().unwrap_or(0);
            if i < 0 {
                // shared statics of another component
                lv_statics(&comps[(-i).to_string()], root, comps)
            } else {
                strs(&root["p"][i.to_string()]).unwrap_or_default()
            }
        }
        _ => vec![],
    }
}

/// Render a rendered-tree node (strings, components, comprehensions) to HTML.
pub fn lv_render(node: &Value, root: &Value, comps: &Value, out: &mut String) {
    match node {
        Value::String(s) => out.push_str(s),
        Value::Number(n) => {
            // component reference
            if let Some(c) = comps.get(n.to_string()) { lv_render(c, root, comps, out); }
        }
        Value::Object(_) => {
            let st = lv_statics(node, root, comps);
            if let Some(rows) = node.get("d").and_then(Value::as_array) {
                for row in rows {
                    for (i, s) in st.iter().enumerate() {
                        out.push_str(s);
                        if let Some(v) = row.get(i) { lv_render(v, root, comps, out); }
                    }
                }
            } else {
                for (i, s) in st.iter().enumerate() {
                    out.push_str(s);
                    if let Some(v) = node.get(i.to_string()) { lv_render(v, root, comps, out); }
                }
            }
        }
        _ => {}
    }
}
