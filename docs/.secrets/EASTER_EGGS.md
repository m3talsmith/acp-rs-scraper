# Easter eggs (3.30.0)

These four surprises are one feature, so they are one minor: **3.30.0**, the next free minor after the work already planned through 3.29.0. 3.6.0–3.9.0 stay unassigned. Nothing from 3.10.0 through 3.29.0 moves.

They are not in the README, `docs/USAGE.md`, the Help topics, or the launch posts. The changelog only says "A few surprises for the curious."

Each animation lasts two seconds (16 frames at 8 fps) and does not loop. Nothing here uses the network. A non-empty `NO_COLOR` with `--color auto`, or `--color never`, and a terminal under 20 columns or 6 rows, drops the colour and the big art: a single asterisk, and the burst still prints its line. `--color always` keeps the colour even when `NO_COLOR` is set.

## 1. Konami

In the terminal UI, type ↑ ↑ ↓ ↓ ← → ← → then `b` `a` (case does not matter).

A short ASCII star burst plays, with the line:

> Be an explosive fucking nebula

Any key skips it, including Ctrl-C, and so does a mouse click or the wheel. That key does not also quit, bookmark, or type. The burst does not start while a text field has focus: the nick filter, a menu text option, the theme editor, login, a search text row, compose title / body / attach / completion, research name / description / note, the command palette, or a confirm box that is waiting for `yes`. The arrows before the last `a` still move the cursor. Only the completing `a` is swallowed.

## 2. `--roar`

```
acp-red-star --roar
```

The flag is `hide = true`, so it is not in `--help`. It prints an ASCII star and one of these lines, chosen from the clock, then `— Haz al Din (@hazaldin)`:

- What a man is made of! Where's the passion in your soul?
- Wake the fuck up!
- We get one life. Why not be an explosion?
- You have one life to live. Be worthy of the cosmic catastrophes all around us.
- You have one fucking life

Those are the roar lines. No others are invented. The command exits before the database, the home directory, and the network. `--roar --tui` still roars and exits. A red ANSI colour wraps the star unless colour is off (`NO_COLOR` with `auto`, or `--color never`). Under 20 columns the star is `*`.

## 3. Search

Searching exactly `one fucking life` (trimmed, any case) in the terminal UI flashes a short starfield, then shows the results as usual. Skipping the flash also reveals the results. The command-line `--search` is unchanged and does not play anything.

## 4. Credits

About is a normal menu item (hint `t`). It shows the name, the version, and a star. The credits are not a menu label.

Open them by either:

- pressing `~` while a text field does not have focus, or
- pressing the header star five times, each within four seconds of the last, or
- with About open, pressing Enter or Space, or clicking the star, five times the same way.

The credits name Neon Lust (Michael's band; the demo music is a Neon Lust song he is writing, guitar by him), Haz al Din (@hazaldin) for the roar clip from IGG | Qizilbash (<https://www.youtube.com/@IGG-QZBSH>), and the contributors. They are signed:

```
For the working class,
~ @m3talsmith
```

The wallpapers in `assets/wallpapers/` are a separate delivery. This feature does not add them.
