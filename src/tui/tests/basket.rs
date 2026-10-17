//! The export basket: x / the + button collect posts, Export › Export basket writes them with --post-ids.

use super::*;
use crate::tui::commands::{Action, CommandId};
use crate::tui::input::on_key;
use crate::tui::state::{Effect, Hit};
use crate::tui::jobs::JobKind;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

fn render(a: &mut App) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let now = std::time::Instant::now() + std::time::Duration::from_secs(5);
    t.draw(|f| crate::tui::widgets::draw(f, a, now)).unwrap();
    let buf = t.backend().buffer().clone();
    (0..40).map(|y| (0..120).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn alice(a: &mut App) { a.select_only(nick_idx(a, "alice")); a.focus = crate::tui::state::Focus::Body; }

#[test]
fn x_adds_and_removes_the_highlighted_post() {
    let mut a = app();
    alice(&mut a);
    let id = a.current_post_id().unwrap();
    on_key(&mut a, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(a.basket, std::slice::from_ref(&id));
    assert!(a.status.back().unwrap().contains("added to the export basket (1)"));
    on_key(&mut a, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(a.basket.is_empty());
}

#[test]
fn cards_and_details_have_a_basket_button() {
    let mut a = app();
    alice(&mut a);
    render(&mut a);
    let (r, _) = *a.hits.iter().find(|(_, h)| *h == Hit::Basket(0)).expect("card button");
    crate::tui::input::on_mouse(&mut a, crossterm::event::MouseEvent { kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: r.x, row: r.y, modifiers: KeyModifiers::NONE });
    let first = a.view().posts[0].post_id.clone();
    assert_eq!(a.basket, std::slice::from_ref(&first));
    assert!(render(&mut a).contains(" ✓ "));
    a.open_post(1);
    render(&mut a);
    assert!(a.hits.iter().any(|(_, h)| *h == Hit::BasketTop));
    a.act(Action::ToggleBasket);
    assert_eq!(a.basket.len(), 2, "the open detail is added");
}

#[test]
fn export_basket_runs_an_export_with_post_ids() {
    let mut a = app();
    a.act(Action::ExportBasket);
    assert!(a.effects.is_empty(), "nothing to export yet");
    a.basket = vec!["ALICE0002".into(), "BOB000001".into()];
    a.act(Action::ExportBasket);
    assert!(a.effects.is_empty(), "needs a path");
    a.menu.menu.set_text(CommandId::Export, "path", "/tmp/picked.json");
    a.menu.menu.set_choice(CommandId::Export, "format", "json,html");
    a.act(Action::ExportBasket);
    let Some(Effect::RunJob(JobKind::Export, argv)) = a.effects.last() else { panic!("{:?}", a.effects) };
    let line = argv.join(" ");
    assert!(line.contains("--export /tmp/picked.json --post-ids ALICE0002,BOB000001 --format json,html"), "{line}");
}

#[test]
fn basket_is_an_export_scope_and_a_view() {
    let mut a = app();
    a.menu.menu.set_text(CommandId::Export, "path", "/tmp/b.json");
    a.menu.menu.set_choice(CommandId::Export, "scope", "Basket");
    let e = a.menu.menu.export_argv_with(&[], None, &[], None).unwrap_err();
    assert!(e.contains("basket is empty"));
    let argv = a.menu.menu.export_argv_with(&[], None, &["ALICE0001".into()], None).unwrap();
    assert!(argv.join(" ").contains("--post-ids ALICE0001"));
    a.basket = vec!["ALICE0003".into(), "ALICE0001".into()];
    a.act(Action::ShowBasket);
    let v = a.view();
    assert!(v.title.contains("Export basket (2)"));
    assert_eq!(v.posts.iter().map(|p| p.post_id.as_str()).collect::<Vec<_>>(), ["ALICE0003", "ALICE0001"], "in the order added");
    a.act(Action::ClearBasket);
    assert!(a.basket.is_empty());
}
