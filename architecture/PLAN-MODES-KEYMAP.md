# Plan: modal input, focus, and keymap

Status: **design.** Nothing here is implemented. It ships before the 3.0 launch. It is lower priority than the features already in flight, and it is not optional for that launch. It lands early enough that people learn these keys first, not the fixed keys the terminal uses today.
Target path: `architecture/PLAN-MODES-KEYMAP.md`.
Companion: [PLAN-3.0-CLIENT.md](./PLAN-3.0-CLIENT.md) (roadmap and §4.6) and [PLAN-SCRAPERS.md](./PLAN-SCRAPERS.md) (service and capability traits).

---

## 0. Decisions

- Input is modal, in the way Neovim modes are modal. The app has one current mode. The mode is set by what is focused. Viewing a post puts the app in post mode, and post mode's keys are then valid. In post mode, `r` replies.
- Global keys use a Ctrl-<key> chord. Every other key is a mode hotkey.
- One object is focused at a time. Tab, Shift-Tab, and a click move focus. The focused widget is drawn with a visible indicator styled through `ratatui-lipgloss`.
- A mode's keys are the capabilities of the focused object's service. A capability that service does not implement has no key. A capability that is implemented but not ready still shows the reason (not captured, not logged in, disabled, or over a cap).
- Bindings live in `~/.acp/keymap.toml`. The app ships a commented default. The user edits that file or the Preferences keymap screen. Both paths reload through one function. A bad file shows the error and does not crash, and the previous map stays loaded.
- Help, tooltips, the action bar, and About render the active binding. They do not embed a key. Docs name the action and point at the keymap.

---

## 1. Focus

Today the terminal keeps a `Focus` value: `Nicks`, `Body`, `Menu`, `Status`, or `Actions`. Modals keep a second focus inside the dialog. Keys are matched in code. This plan keeps those panes and makes focus the thing that selects the mode.

- The screen has a focus order. Tab moves to the next widget. Shift-Tab moves to the previous one. A click focuses the widget under the pointer.
- A Ctrl chord does not move focus. It runs the global action.
- A modal takes the focus order until it closes. Tab does not escape the modal.
- Inside a pane, focus can sit on the object that pane is showing: a nick, a post, a menu row, a job, an action-history step. That object is what the mode's keys act on.
- The indicator is a theme border or bar from `ratatui-lipgloss`. It is not a one-off colour in the widget.

The mode is the kind of the focused object.

| Focus | Mode |
|---|---|
| A post or a comment in the body | `post` |
| The nick bar | `nick` |
| The menu bar | `menu` |
| The status bar | `status` |
| The action-history bar | `actions` |
| A text field (compose title, body, attach, search, a preferences field) | `insert` |
| A modal that is not a text field | the modal's own mode (`confirm`, `help`, `picker`) |

`insert` is the mode in which typed letters go into the field. Esc leaves it and restores the mode of the pane underneath. Whether compose should work that way, or stay a special case, is open (§6).

---

## 2. Modes and capabilities

The services plan is the source of capability ids. This plan does not invent a second list.

- Post mode on a Red Star post offers the Red Star capabilities that are ready. The same mode on an X post offers X's. The letter can be the same (`r` replies) because the action id is `reply` and the service is whichever object is focused.
- The UI and the command line offer an action only when the focused or selected service implements that capability. A gated or not-ready action stays visible and shows the reason. A missing capability is omitted, so it has no key and no menu row.
- An action never crosses services. The keymap names an action id. The dispatcher sends it to the focused object's service.
- Global actions are not capabilities. Bookmarks, site search, edit history, help, quit, the source switcher, keymap reload, and plugin reload are globals. Their bindings are Ctrl chords.

The source switcher's default chord is `ctrl-]` (`view.source` in the keymap). It is a binding in the file, not a constant in the widgets. A user can change it.

Plugin reload's action id is `plugins.reload`. Preferences › Services and this action call one loader. The default chord in the keymap file is `ctrl-p`. A remap changes the chord. The widgets do not hardcode it. Today's command palette also opens on Ctrl-P and on Ctrl-Space. This default gives `ctrl-p` to plugin reload. Ctrl-Space stays the palette.

---

## 3. The keymap file

`~/.acp/keymap.toml` is the only keymap. Missing file: copy the shipped default, which is commented, and load that. The default lives in the repo at `keymap/default.toml` and is installed next to the other defaults.

```toml
# ~/.acp/keymap.toml
# One key, one action, in each table. Reload from the app after editing.

[global]
"ctrl-]" = "view.source"     # which service's rows are on screen
"ctrl-h" = "help.open"
"ctrl-q" = "app.quit"
"ctrl-k" = "keymap.reload"
"ctrl-p" = "plugins.reload"  # pick up plugins placed in ~/.acp/plugins

[mode.post]
"r" = "reply"
"q" = "quote"

[mode.insert]
# Printable keys go to the field. These chords still work.
"esc" = "insert.leave"
"ctrl-k" = "keymap.reload"
"ctrl-p" = "plugins.reload"
```

That block is the shape, not the whole default. The shipped file lists every action the app has today, each with a comment naming the old fixed key.

Rules for the loader:

- Action ids are a fixed set the app knows. An unknown id is an error.
- A key used twice in the same table is a conflict. The error names both lines.
- A mode key and a Ctrl global do not collide: `r` and `ctrl-r` are different keys.
- On error the message includes the file path and the line. The app keeps the map it had. It does not exit.
- Preferences › Keymap edits this same file and calls this same loader. A reload action (`keymap.reload`, default `ctrl-k`) does too. Plugin reload (`plugins.reload`, default `ctrl-p`) is a different action: Preferences › Services and that chord call the plugin loader. The chord is remappable. The widgets do not hardcode it.

Help, a tooltip, the action bar, and About ask the loaded map for the binding of an action in the current mode, and fall back to the global table. A missing binding shows the action with no key, not a hardcoded letter.

`docs/USAGE.md`, the README, and Getting Started name actions. Where they show a key, they show it as the default in `keymap/default.toml` and say the keymap wins after a remap. They do not describe a key the default file does not have.

---

## 4. Migration

The current key match and the `Focus` enum stay the starting point. The work is to route those keys through the map.

- Keys that act on the focused object move into that object's mode, keeping the letter when it still means the same thing (`r` replies in post mode). Bookmark, site search, and edit history stay global actions. The default file binds each one and comments the old key.
- Keys that work from any pane (help, quit, source, keymap reload, plugin reload) become Ctrl chords. F1 may remain a second binding for help in the default file, commented as a leftover of today's `?` and F1.
- Confirm dialogs keep Cancel as the default. `y` and `n` in the confirm mode are bindings, still with Cancel focused first.
- Cucumber steps that press today's keys keep passing against the default file. New steps cover a remapped file.

---

## 5. Tests

Mocked Gherkin only. No live site. Throwaway `HOME`.

- Focus starts on the nick bar. Tab moves to the body. The mode becomes `post` when a post is focused, and `r` replies.
- Shift-Tab moves back. A click on the status bar sets `status` mode and draws the focus indicator.
- Rewrite `r` in `[mode.post]` to `g`, reload, and `r` does nothing while `g` replies. The tooltip shows `g`.
- A duplicate key in one table, and a broken line, each show an error, leave the previous map in use, and leave the process running.

---

## 6. Open questions

1. **Which-key popup.** After a prefix with more than one continuation, should a small map of the next keys appear, or is the Help topic enough?
2. **Counts.** Is a Vim-style count (`10j`) in scope, or are motions always one step?
3. **Insert.** Should every text field be `insert` mode (letters type, Esc leaves), or does compose keep its own keys the way it does today? This plan sketches `insert` and does not lock it.
4. **Mouse and Tab.** Must the click order and the Tab order be the same list, or may a click focus a widget Tab skips?
5. **About.** About is a modal. Does it only inherit global Ctrl chords, or does it have its own mode table for the three segments?
