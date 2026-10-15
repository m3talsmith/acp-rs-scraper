//! Unit tests for mention matching.

use super::*;

#[test]
fn word_boundaries() {
    assert!(word_match("hello haz world", "haz"));
    assert!(word_match("Haz,", "haz"));
    assert!(word_match("@haz said", "haz")); // 'h' after @ — @ is not a word char
    assert!(!word_match("hazard", "haz"));
    assert!(!word_match("ahaz", "haz"));
    assert!(word_match("Jane Doe wrote", "Jane Doe"));
    assert!(!word_match("Jane Does", "Jane Doe"));
    assert!(!word_match("x", ""));
}

#[test]
fn mention_node_by_id_and_label() {
    let body = json!({"type":"doc","content":[
        {"type":"paragraph","content":[
            {"type":"mention","attrs":{"id":"u1","label":"@haz"}},
            {"type":"text","text":" hello"}
        ]}
    ]});
    let post = json!({"id":"p1","body": body});
    let t = MentionTarget { nick: "haz".into(), user_id: Some("u1".into()), display_name: Some("Hazel".into()) };
    let m = match_post(&post, &t);
    assert!(m.contains(&MatchType::Mention));
    // mention node has no text child; plain-nick match needs body text
    assert!(!m.contains(&MatchType::Nick));
    // display name not in text
    assert!(!m.contains(&MatchType::DisplayName));
}

#[test]
fn plain_nick_and_display() {
    let post = json!({"id":"p1","body":{"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"Thanks haz and Hazel Smith for this"}]}
    ]},"title":""});
    let t = MentionTarget { nick: "haz".into(), user_id: None, display_name: Some("Hazel Smith".into()) };
    let m = match_post(&post, &t);
    assert!(!m.contains(&MatchType::Mention));
    assert!(m.contains(&MatchType::Nick));
    assert!(m.contains(&MatchType::DisplayName));
}

#[test]
fn short_display_name_skipped() {
    let post = json!({"id":"p1","content":"Jo is here", "body": Value::Null, "title":""});
    let t = MentionTarget { nick: "someone".into(), user_id: None, display_name: Some("Jo".into()) };
    let m = match_post(&post, &t);
    assert!(!m.contains(&MatchType::DisplayName));
}
