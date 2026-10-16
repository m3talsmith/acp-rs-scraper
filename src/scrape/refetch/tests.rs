//! Unit tests for picking and completing truncated rows.

use super::*;

#[test]
fn completed_body_only_when_different() {
    assert_eq!(completed_body("short", "short\n\nand more"), Some("short\n\nand more".into()));
    assert_eq!(completed_body("a\nb", "a\n\nb"), Some("a\n\nb".into()));
    assert_eq!(completed_body("same", "same"), None);
    assert_eq!(completed_body("kept", ""), None);
}

#[test]
fn pending_rows_skip_full_and_filter_nicks() {
    let dir = std::env::temp_dir().join(format!("acp-refetch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("rs.database");
    let _ = std::fs::remove_file(&db);
    let c = Connection::open(&db).unwrap();
    ensure_posts_schema(&c).unwrap();
    c.execute_batch("INSERT INTO posts (post_id,parent_post_id,nick,title,content,posted_at,body_state) VALUES
        ('T1','','Haz','t','x','1','truncated'), ('R1','T1','haz','','y','2','unknown'),
        ('R2','T1','haz','','z','3','full'), ('O1','','bob','','w','4','truncated');").unwrap();
    let all = pending_bodies(&db, &[]).unwrap();
    assert_eq!(all.len(), 3);
    let haz = pending_bodies(&db, &["@haz".into()]).unwrap();
    let ids: Vec<_> = haz.iter().map(|p| (p.post_id.as_str(), p.thread_id.as_str())).collect();
    assert_eq!(ids, vec![("T1", "T1"), ("R1", "T1")]);
    let _ = std::fs::remove_dir_all(&dir);
}
