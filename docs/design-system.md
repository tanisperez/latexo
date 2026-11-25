# Design system

The readable summary of the design, from the Artifact
(https://claude.ai/artifact/0cb8a17c-326b-4bd6-a813-e8d2f0e6212a). Once built, the code is the
source of truth: `theme/` for colors and metrics, `views/` for the screens.

## Tokens

Latexo uses telaio's default tokens unchanged (`Tokens::DEFAULT_LIGHT` and `DEFAULT_DARK`, the
`project/ds/telaio/tokens.json` of the Artifact). The values the screens lean on:

| Token | Light | Dark | Used for |
| --- | --- | --- | --- |
| `bg` | `#fcfcfd` | `#15161a` | the window |
| `surface` | `#f4f5f7` | `#1b1c21` | title bar, side bar, chart wells, table header |
| `raised` | `#ffffff` | `#23252c` | cards |
| `border` | `#e3e5ea` | `#2c2e36` | card borders, row separators, grid lines |
| `accent` | `#4f57d8` | `#5b63e6` | the first series, bars |
| `accent_subtle` | `#e8e9fd` | `#272b4d` | icon tiles, the selected entry |
| `warning` | `#9a6500` | `#e8b75a` | 80 °C or more, the threshold lines |
| `danger` | `#cf3535` | `#f27171` | 95 °C or more, End process |
| `success` | `#1d8657` | `#4fc88f` | the Live badge, sensors available |

Derived in `theme::LatexoColors`, as the design does:

- `series_secondary`: the accent at 42 % (light) or 45 % (dark) opacity, the second series of a
  stacked chart (system time, writes, upload).
- `dot_off`: an empty dot of the dotted charts and an empty heat map cell.
- `temperature(celsius)`: accent, warning from 80 °C, danger from 95 °C.

## Type

Geist and Geist Mono from telaio. Screen title 20 px semibold; card title 13 px semibold; big
value 28 px (24 px in the GPU and fan cards, 18 to 20 px in the core tiles); rows 12.5 px;
labels and legends 12 px; axis labels 10.5 px; side bar group labels 11 px uppercase. Numbers in
tables and rows are in Geist Mono.

## Layout (`theme/metrics.rs`)

- Title bar 40 px, on `surface`, with the Settings button (28 px) and the window controls.
- Side bar 232 px, 12 px padding, rows 30 px with the screen's live value at the right, the
  Hardware and System groups, the Live badge and About at the bottom.
- Content padding 24 px by 28 px, 16 px between cards. Cards: 16 px padding, 10 px radius, a
  28 px icon tile.
- Overview: three columns of resource cards (value, line, chart in a well), then the busiest
  processes (two columns wide) and the power card.
- Detail screens: a big chart card (axis labels at the left, 112 px chart, time labels under
  it), then a list of bars, an optional list of processes and a Details card.
- Processes: a table with a 32 px header and 30 px rows, sortable headers, a footer with the
  count and the sort.
- Settings: two columns of cards with 42 px rows (label, description, control at the right).

## Charts

Four styles, chosen in the header of every screen and in Settings, applied to every chart:

- **Line**: a 1.5 px line and its area at 20 % opacity; a stacked second series fills the band
  above in `series_secondary`.
- **Bars**: one bar per bucket, about 4 px pitch.
- **Steps**: the line with a flat value between samples.
- **Dots**: columns of 6 px dots with a 2 px gap, the dotted grid of the first design.

Grid: dashed lines at 0, 50 and 100 %. Thresholds: a dashed warning line; bars and dots over it
turn warning. Ranges: 1, 5 and 15 minutes and 1 hour.

## Cores

Four views of the design, chosen in the header: Bars (usage and temperature per core, clock),
Heat map (usage over time in seven steps of the accent), Charts (a small chart per core with its
temperature badge) and Processor (tiles tinted by usage, a warning ring at 80 °C, and the
temperature of each core as columns).

## Not built yet

The tray flyout and the floating pill, the fan curve and profiles, and the 24 h and 7 day history
of the design's Settings (see `AGENTS.md`, Pending decisions).
