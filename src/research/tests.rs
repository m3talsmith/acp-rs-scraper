//! Research storage, local files, readable text and the export section.

use super::*;
use std::collections::HashSet;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("acp-research-{tag}-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn item(kind: ItemKind, r: &str) -> NewItem { NewItem { kind: Some(kind), r#ref: r.into(), title: format!("t {r}"), ..NewItem::default() } }

#[test]
fn names_are_checked() {
    assert_eq!(check_name("  Strikes 2026 ").unwrap(), "Strikes 2026");
    assert!(check_name("").is_err());
    assert!(check_name("   ").is_err());
    assert!(check_name("bad\nname").is_err());
    assert!(check_name(&"x".repeat(81)).is_err());
    assert!(check_name(&"x".repeat(80)).is_ok());
}

#[test]
fn projects_are_created_listed_and_found_case_insensitively() {
    let d = tmp("crud");
    let c = research_conn(&d.join("db.sqlite")).unwrap();
    let id = create_project(&c, "Strikes", "rail and port").unwrap();
    assert!(create_project(&c, "strikes", "").is_err(), "duplicate names are refused");
    assert_eq!(project_id(&c, "STRIKES").unwrap(), Some(id));
    assert_eq!(get_or_create(&c, "strikes", None).unwrap(), (id, false));
    let (id2, created) = get_or_create(&c, "Unions", Some("history")).unwrap();
    assert!(created && id2 != id);
    let v = list_projects(&c).unwrap();
    assert_eq!(v.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Unions", "Strikes"], "newest first");
    assert_eq!(get_project(&c, "unions").unwrap().unwrap().description, "history");
    assert!(get_project(&c, "nope").unwrap().is_none());
}

#[test]
fn items_are_deduplicated_and_notes_updated() {
    let d = tmp("items");
    let c = research_conn(&d.join("db.sqlite")).unwrap();
    let p = create_project(&c, "P", "").unwrap();
    assert!(add_item(&c, p, &item(ItemKind::Post, "A1")).unwrap());
    assert!(add_item(&c, p, &item(ItemKind::WebUrl, "https://e.x/")).unwrap());
    let mut again = item(ItemKind::Post, "A1");
    again.note = "key source".into();
    assert!(!add_item(&c, p, &again).unwrap(), "same kind + ref is not added twice");
    let v = project_items(&c, p).unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].r#ref, "A1");
    assert_eq!(v[0].note, "key source");
    assert_eq!(v[1].kind, ItemKind::WebUrl);
    assert_eq!(list_projects(&c).unwrap()[0].items, 2);
}

#[test]
fn deleting_a_project_removes_its_items() {
    let d = tmp("cascade");
    let c = research_conn(&d.join("db.sqlite")).unwrap();
    let p = create_project(&c, "P", "").unwrap();
    add_item(&c, p, &item(ItemKind::Post, "A1")).unwrap();
    c.execute("DELETE FROM research_projects WHERE id = ?1", [p]).unwrap();
    let n: i64 = c.query_row("SELECT COUNT(*) FROM research_items", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn kinds_round_trip() {
    for k in [ItemKind::Post, ItemKind::Reply, ItemKind::SearchResult, ItemKind::LocalFile, ItemKind::WebUrl] {
        assert_eq!(ItemKind::parse(k.as_str()), Some(k));
    }
    assert!(ItemKind::Reply.is_post() && ItemKind::SearchResult.is_post() && !ItemKind::WebUrl.is_post());
    assert_eq!(ItemKind::parse("telegram"), None);
}

#[test]
fn mime_types_by_extension() {
    assert_eq!(mime_for(Path::new("a/notes.MD")), "text/markdown");
    assert_eq!(mime_for(Path::new("x.pdf")), "application/pdf");
    assert_eq!(mime_for(Path::new("x.jpeg")), "image/jpeg");
    assert_eq!(mime_for(Path::new("noext")), "application/octet-stream");
}

#[test]
fn local_files_keep_metadata_and_text_without_copying() {
    let d = tmp("file");
    let f = d.join("notes.txt");
    std::fs::write(&f, "picket line at 6am\n").unwrap();
    let it = file_item("P", &f, false, "from the meeting").unwrap();
    assert_eq!(it.kind, Some(ItemKind::LocalFile));
    assert_eq!(it.title, "notes.txt");
    assert_eq!(it.note, "from the meeting");
    assert_eq!(it.content, "picket line at 6am\n");
    assert_eq!(it.meta["size"], 19);
    assert_eq!(it.meta["mime_type"], "text/plain");
    assert_eq!(it.meta["copied"], false);
    assert_eq!(it.local_path, std::fs::canonicalize(&f).unwrap().display().to_string());
    assert!(file_item("P", &d.join("missing.txt"), false, "").is_err());
    assert!(file_item("P", &d, false, "").is_err(), "directories are refused");
}

#[test]
fn binary_files_have_no_text() {
    let d = tmp("bin");
    let f = d.join("pic.png");
    std::fs::write(&f, [0x89, b'P', b'N', b'G']).unwrap();
    let it = file_item("P", &f, false, "").unwrap();
    assert!(it.content.is_empty());
    assert_eq!(it.meta["mime_type"], "image/png");
}

#[test]
fn readable_text_skips_navigation_and_scripts() {
    let html = r#"<html><head><title>Rail strike</title><script>var x = 1;</script></head><body>
        <nav><p>Home | About</p></nav>
        <article><h1>Rail workers walk out</h1><p>Thousands   joined the
        <a href="/p">picket</a>.</p><ul><li>Day one</li><li>Day <b>two</b></li></ul><blockquote>We stay out.</blockquote>
        <pre>code  kept</pre><footer><p>© paper</p></footer></article></body></html>"#;
    let (title, text) = readable(html);
    assert_eq!(title, "Rail strike");
    assert_eq!(text, "# Rail workers walk out\n\nThousands joined the picket.\n\n- Day one\n\n- Day two\n\n> We stay out.\n\n```\ncode  kept\n```");
    assert!(!text.contains("Home") && !text.contains("paper") && !text.contains("var x"));
}

#[test]
fn readable_prefers_og_title_and_falls_back_to_body() {
    let (t, x) = readable(r#"<html><head><meta property="og:title" content="OG"><title>T</title></head><body><p>Hello</p></body></html>"#);
    assert_eq!((t.as_str(), x.as_str()), ("OG", "Hello"));
    let (t, _) = readable("<html><body><h1>Only heading</h1></body></html>");
    assert_eq!(t, "Only heading");
}

#[test]
fn post_titles_are_one_short_line() {
    let p = post("alice", "", "line one\nline   two");
    assert_eq!(post_title(&p), "@alice: line one line two");
    let long = post("b", &"y".repeat(100), "");
    assert!(post_title(&long).ends_with('…'));
}

fn post(nick: &str, title: &str, content: &str) -> Post {
    Post { post_id: "A1".into(), parent_post_id: String::new(), in_reply_to_id: String::new(), nick: nick.into(), title: title.into(),
        content: content.into(), posted_at: String::new(), media: Vec::new(), truncated: false, body_json: String::new(), score: None }
}

fn sample() -> (ResearchProject, Vec<ResearchItem>) {
    let p = ResearchProject { id: 1, name: "Strikes".into(), description: "rail".into(), created_at: "2026-10-17".into(), items: 2 };
    let items = vec![
        ResearchItem { id: 7, kind: ItemKind::Post, r#ref: "A1".into(), title: "@a: hi".into(), note: "n".into(), meta: json!({}),
            content: String::new(), local_path: String::new(), added_at: "2026-10-17".into() },
        ResearchItem { id: 8, kind: ItemKind::WebUrl, r#ref: "https://e.x/a".into(), title: "Page <b>".into(), note: String::new(),
            meta: json!({"content_type": "text/html"}), content: "# Head\n\nBody".into(), local_path: "/nonexistent/a.html".into(), added_at: "2026-10-17".into() },
    ];
    (p, items)
}

#[test]
fn export_json_lists_every_item_with_its_file() {
    let (p, items) = sample();
    let v = research_json(&p, &items, &|i| (i.kind == ItemKind::WebUrl).then(|| "files/a.html".to_string()));
    assert_eq!(v["name"], "Strikes");
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    assert_eq!(v["items"][0]["kind"], "post");
    assert_eq!(v["items"][0]["file"], Value::Null);
    assert_eq!(v["items"][1]["file"], "files/a.html");
    assert_eq!(v["items"][1]["content"], "# Head\n\nBody");
}

#[test]
fn attachments_only_for_files_that_exist() {
    let d = tmp("att");
    let f = d.join("a.txt");
    std::fs::write(&f, "x").unwrap();
    let (_, mut items) = sample();
    assert!(item_attachments(&items).is_empty());
    items[1].local_path = f.display().to_string();
    let a = item_attachments(&items);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].post_id, "research:8");
    assert_eq!(a[0].url, "https://e.x/a");
    assert_eq!(a[0].mime_type, "text/html");
}

#[test]
fn html_section_escapes_and_links_posts() {
    let (p, items) = sample();
    let v = research_json(&p, &items, &|_| None);
    let ids: HashSet<String> = ["A1".to_string()].into();
    let h = crate::export::html::research_html(&v, &ids);
    assert!(h.contains("Research: Strikes"));
    assert!(h.contains("Page &lt;b&gt;"), "{h}");
    assert!(h.contains("#post-A1") || h.contains("post-A1"), "{h}");
}
