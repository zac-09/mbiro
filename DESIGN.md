# Mbiro: design spec

The look is called **Instrument panel**: near-black, one acid-lime accent, condensed uppercase headings, monospaced data. It should feel like a piece of measuring equipment, not a consumer web app.

The designed screens are in `design/reference/` as HTML. Each file's inline styles are the source of truth for sizes, spacing and colours; copy exact values from them. They use a template runtime (`{{holes}}`, `<sc-for>`, a `Component` class), so read them as markup reference and rebuild them as React components. Do not try to run them.

| File | Screen |
| --- | --- |
| `Main.dc.html` | Main window |
| `AddVideo.dc.html` | Add download, with the video quality picker |
| `MenuBar.dc.html` | Menu bar popover |
| `ExtPopup.dc.html` | Extension popup, app connected |
| `ExtOffline.dc.html` | Extension popup, app not found |
| `ExtToast.dc.html` | In-page notice after a download is caught |
| `ExtOptions.dc.html` | Extension settings: capture rules |
| `Logo.dc.html` | Logo sheet: icon, lockups, menu bar glyph |

Settings and onboarding are not designed yet. Build them from the tokens and components below, and keep them plain.

## Logo

The mark is five bars hanging from a top line that read as an M, with one block falling from the centre bar. Files are in `design/logo/`. Never redraw it; use the files.

| File | Use |
| --- | --- |
| `app-icon.svg`, `app-icon-1024.png` | The app icon: dark mark on an accent tile. Feed the PNG to `tauri icon` |
| `app-icon-dark.svg`, `app-icon-dark-1024.png` | Alternate icon: accent mark on a dark tile |
| `mark-lime.svg`, `mark-dark.svg` | The bare mark, for in-app use and the extension icon |
| `menubar-template.svg` | Black-only mark for the macOS menu bar; mark it as a template image so the system tints it |
| `lockup-lime.svg`, `lockup-dark.svg` | Mark plus wordmark, with the letters converted to outlines |

Rules: keep clear space of one bar-width around the mark; use it only in `accent`, `bg` dark, black or white; do not animate, outline, rotate or recolour it. Below 16px use the mark alone, never the lockup. The in-app wordmark may stay as live text in Barlow Condensed 700.

## Colour tokens

| Token | Value | Use |
| --- | --- | --- |
| `bg` | `#0C0D0E` | Every surface |
| `bg-field` | `#141618` | Text inputs |
| `bg-raised` | `#1B1E20` | Placeholders, thumbnails |
| `ink` | `#F2F4F5` | Primary text |
| `ink-muted` | `#A7ADB2` | Labels, detail lines, paused values |
| `ink-soft` | `#D4D8DB` | Body paragraphs |
| `accent` | `#C6F432` | Speed readout, active blocks, primary buttons, selected state |
| `accent-dim` | `#5E7417` | Completed progress blocks |
| `accent-wash` | `#1A1F0C` | Background of a selected option |
| `seg-empty` | `#24272A` | Unfilled progress blocks |
| `seg-paused` | `#7B8187` | Paused progress blocks |
| `rule-strong` | `#2A2D30` | Section dividers |
| `rule` | `#1F2224` | Row dividers |
| `border` | `#3C4044` | Buttons, inputs |
| `danger` | `#FF5A3C` | Failed progress blocks, status squares |
| `danger-ink` | `#FF9A80` | Failed text and button borders |

Text on `accent` is always `#0C0D0E`. Never use `danger` (`#FF5A3C`) for small text; use `danger-ink`.

## Type

Two families, loaded locally in the app (bundle the font files; do not fetch from a CDN at runtime).

- **Barlow Condensed** (600, 700): the wordmark, headings, the speed readout, percentages, primary button labels. Always uppercase, letter-spacing 0.06em to 0.16em.
- **IBM Plex Mono** (400, 500, 600): everything else. Labels and button text are uppercase at 11.5 to 12px with 0.06em to 0.1em letter-spacing. File names and paths keep their own case at 13 to 13.5px.

Use tabular numerals everywhere (`font-variant-numeric: tabular-nums`) so live numbers do not jitter.

| Element | Face | Size | Weight |
| --- | --- | --- | --- |
| Speed readout, main window | Barlow Condensed | 156px, line-height 0.86 | 700 |
| Speed readout, menu bar | Barlow Condensed | 58px | 700 |
| Screen heading | Barlow Condensed | 40 to 46px | 700 |
| Row percentage | Barlow Condensed | 36px | 600 |
| Setting name, option name | Barlow Condensed | 21 to 22px | 600 |
| Primary button label | Barlow Condensed | 18 to 20px | 700 |
| File name | IBM Plex Mono | 13.5px | 500 |
| Label, detail line, secondary button | IBM Plex Mono | 11.5 to 12px | 500 |

## Shape and spacing

- Corner radius is 2px on buttons, inputs and panels. Progress blocks are square. Nothing is rounded beyond 2px.
- No shadows, no gradients, no blur. Separation comes from 1px rules.
- All interactive targets are at least 44px tall.
- Window padding is 22 to 36px. Rows are 78px tall with 1px `rule` dividers.

## Components

**Segment bar.** Progress is a row of square blocks, not a continuous bar: 32 blocks in the main window, 28 in the menu bar, 2px gaps, each block 10px wide and 26px tall (16px tall in the menu bar). Filled count is `round(percent / 100 * blocks)`, minimum 1. Colours: active `accent`, paused `seg-paused`, done `accent-dim`, failed `danger`, empty `seg-empty`. On an active download the last filled block blinks.

**Speed readout.** The total speed in MB/s, one decimal, in the accent colour, with a muted "MB/S" unit beside it. It is the largest thing on the main window and updates once a second.

**Buttons.** Primary: `accent` fill, dark text, Barlow Condensed. Secondary: transparent, 1px `border`, mono uppercase. One primary button per screen.

**On/off switch.** Two joined buttons labelled ON and OFF. The active side is filled with `accent`; the other is outlined. Use `aria-pressed`.

**Tabs.** Mono uppercase with the count in the label ("ACTIVE 3"). The selected tab has a 2px `accent` underline.

**Status square.** An 8px square before a status label: `accent` and blinking when connected or active, `danger` and still when not.

**Selected option.** 1px `accent` border and `accent-wash` background, as in the quality picker.

## Download row states

| State | Blocks | Percentage | Detail line | Button |
| --- | --- | --- | --- | --- |
| Downloading | `accent`, last one blinks | `ink` | speed and time left, `8.1 MB/S · 02:15` | PAUSE |
| Paused | `seg-paused` | `ink-muted` | `PAUSED` | RESUME |
| Done | all `accent-dim` | `ink-muted`, 100% | `DONE · 10:42` | SHOW FILE |
| Failed | `danger` | `danger-ink` | reason in `danger-ink` | RETRY, `danger-ink` border |

Time left is `MM:SS` under an hour. Sizes read `1.8 GB / 2.9 GB`.

## Motion

Keep motion to three things, all cheap:

1. The last filled block of an active download blinks (0.9s, two steps).
2. The speed readout drops in on window open (0.7s).
3. The in-page notice shows a 6-second draining bar before it dismisses.

Honour `prefers-reduced-motion`: no blink, no drop, no drain.

## Layout rules

- The main window has no sidebar. Filters and search sit in the top bar. A download's category shows in its detail line.
- The main window is fluid: at narrow widths the top bar and rows wrap; nothing scrolls sideways.
- The extension popup is 340px wide. The menu bar popover is 360px wide.

## Accessibility

- Real `button`, `input` and `label` elements. Icon-only buttons need `aria-label`.
- Body and label text must keep at least 4.5:1 contrast against `bg`; the tokens above do.
- State is never shown by colour alone: every state also has a text label.
- Full keyboard operation of the list: arrow keys move, space pauses or resumes, delete removes after a confirm.

## Copy

- Labels are short and uppercase. Messages are plain sentences in normal case.
- Errors say what happened and what to do, for example "The server closed the connection. Retry, or check the link."
- No video site is named anywhere in the UI, the extension, the help pages or the website.
