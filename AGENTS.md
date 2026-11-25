# AGENTS.md

Guidance for AI agents working on Latexo.

The rules shared by every Rust desktop app (design first, stack, telaio, performance,
translations, logging, CLI, docs, releases, packaging, CI, testing) are imported below. This file
only adds what is specific to Latexo, and wins where the two differ.

@~/.claude/rust/RULES.md

## Project

Latexo ("heartbeat" in Galician) is a native system monitor for **Linux and Windows**, written in
Rust with `gpui` (through `gpui-kit`) and telaio. It shows the CPU and each core, the GPU,
memory, disks, network, temperatures, fans, power and energy, the processes, and alert rules,
with charts in four styles. Postino (`~/dev/postino`) is the reference implementation and ted
(`~/dev/ted`) the closest sibling: copy their skeleton, conventions and patterns, and rename.

**The UI is the product.** It must look as good as the design and stay light: a monitor that
costs noticeable CPU defeats its purpose.

It is a desktop project, unrelated to the Expo mobile apps in `~/dev`. Do not reuse the
Expo/React Native conventions here.

## Decisions taken

- Name: **Latexo**. Binary `latexo`, app id `codes.tanis.latexo`, env prefix `LATEXO_`,
  repository `tanisperez/latexo`, website `latexo.tanis.codes` (not built yet).
- Platforms: **Linux and Windows only** (decided on 2026-11-25). No macOS build, bundle or
  workflow job; where the shared rules mention macOS, skip it.
- License: Apache-2.0, as Postino.
- Stack: Rust (edition 2024, stable toolchain) + `gpui-kit` 0.10.0 + telaio (`telaio-ui`,
  `telaio-app`, `telaio-update`) pinned to one tag in `[workspace.dependencies]` (`v0.32.0`
  today). `cargo tree -d --depth 0 | grep gpui` must stay empty.
- **Readings: `sysinfo` 0.40** (CPU, memory, disks, networks, processes, temperature sensors and
  GPUs through NVML, Vulkan and the Windows performance counters), plus Linux `/sys` readers of
  our own for what sysinfo lacks: fans (`hwmon`), the battery (`power_supply`), the processor
  package power (`powercap`, root only on most distributions), the user and kernel split
  (`/proc/stat`) and the temperature of each core (`topology/core_id` matched with `coretemp`).
- **Sampling runs on a thread of its own** (`latexo-sampler`), not on gpui's background
  executor: sysinfo's GPU handles are not `Send`. It sends one sample per interval through an
  `async-channel`; the interval is an `AtomicU64` the settings update.
- **Settings inside the window**, as a screen of the side bar, not the usual modal (asked by the
  user on 2026-11-25). About is a screen too.
- Languages of the UI: **English** (source and fallback), **Spanish**, **Galician** and
  **Italian**.
- History: one hour in memory (7200 samples, the longest range at 0.5 s). Chart ranges 1, 5 and
  15 minutes and 1 hour. Nothing of the history is written to disk; Settings exports it as CSV.
- Energy: the measured power (the battery discharge on a laptop on battery, else the package and
  GPU power when readable) added up into watt hours today and this month, saved in
  `energy.toml`, priced with the `energy_price` setting. Nothing is estimated.
- Alerts: rules in `settings.toml` (`[[alert]]`), checked on every sample by
  `latexo_core::alerts::Watcher`; a rule fires once when its condition holds for its duration
  and again only after it clears. They show as in-window notifications and in the history of the
  Alerts screen. No OS notifications yet.
- Start with the session: an XDG autostart entry on Linux, an `HKCU\...\Run` value written with
  `reg.exe` on Windows (no registry crate, no `unsafe`).
- Design: the prototype lives in a claude.ai Artifact (Design canvas),
  https://claude.ai/artifact/0cb8a17c-326b-4bd6-a813-e8d2f0e6212a (all screens in light and
  dark, the four core views, the chart styles, the tray and the floating pill). UI changes are
  designed there first; read its pages with the Artifact tool, never a local copy.

## Pending decisions (ask before choosing)

- **Tray icon and the floating pill** of the design: gpui has no tray support. Needs a tray crate
  (and its own event loop on Linux) or a small always-on-top window for the pill. Not built.
- **Fan curves and fan control**: the design's profiles need a board driver; Latexo only reads.
- **Persistent history** (24 h, 7 days, as the design's Settings offered): needs a small on-disk
  store. Not built; the setting was left out instead of shown without effect.
- **Estimated power** of the parts without a sensor (memory, disks, board): left out, nothing is
  estimated today.
- **Windows sensors beyond sysinfo** (CPU package power, fans, more temperatures) need a kernel
  driver such as the one of LibreHardwareMonitor. Not planned without the user.
- **Running elevated** to read root-only counters (RAPL on Linux): not done; the Sensors section
  of Settings explains it.
- Website, user docs and package repositories: copy Postino's `site/` when the user asks.

## Architecture

A Cargo workspace of small crates, UI kept thin. Dependency direction, no cycles, no crate
depends on `latexo-app`:

```
latexo-app ──> latexo-sensors ──> latexo-core
    ├────────> latexo-core
    ├────────> latexo-cli (standalone, depends on no other latexo crate)
    └────────> telaio-ui, telaio-app, telaio-update
```

- `latexo-core`: the domain model, no IO and no gpui. `Sample` (one reading of the whole
  machine), `History` (every series aligned on one ring of times, `NaN` for gaps), the process
  query (sort, filter, search, process tree), the alert rules and their `Watcher`, the energy
  account and the units.
- `latexo-sensors`: reads the machine. `Sampler::new` opens the handles, `Sampler::sample`
  returns one `Sample` with rates over the time since the previous call. `linux` holds the `/sys`
  readers (each takes the root folder, so tests use fixtures), `classify` groups sensors by their
  label. `end_processes` ends processes (SIGTERM on Linux, TerminateProcess on Windows). The
  `probe` example prints what a machine reports: `cargo run -p latexo-sensors --example probe`.
- `latexo-cli`: `-h/--help`, `-V/--version`, `IDENTITY`, `build.rs` with the commit, as
  `ted-cli`, without paths (Latexo opens no files).
- `latexo-app`: the gpui binary (`latexo`).
  - `state/`: settings, the `Monitor` (info, newest sample, history, energy, alert log), value
    formatting in the active language, navigation, autostart, the updater gate, locale tests.
  - `theme/`: telaio's default tokens (the design uses them unchanged), `LatexoColors` (the
    translucent second series, the empty dot, the temperature colors) and `metrics`.
  - `views/`: `root` (title bar, side bar, header, sampling), `chart` (the domain chart widget,
    painted with `canvas`), `charts` (how screens build charts), one module per screen and
    `widgets` (card, rows, legend, badges).

Everything except `latexo-app` is testable with `cargo test`, with no window and no GPU.

## App-specific rules

- **Colors only from the palette** (`cx.palette()` and `LatexoColors`), sizes only from
  `theme::metrics`.
- **An idle Latexo redraws once per sample and does nothing else**: no timers, no animations.
  The sampling thread sleeps between samples; the UI task sleeps on the channel. Charts never
  draw more points than pixels (`views/chart.rs` buckets at paint time).
- **Never a made up value**: a reading the platform does not give is `None` and shows as "Not
  available" or is left out, with a note that explains why when it helps (Fans on Windows, the
  package power on Linux).
- New strings go to `crates/latexo-app/locales/<area>.yml` in the four languages at once.

## Testing and debug hooks

- Unit tests in every crate; `/sys` readers with fixtures in a temporary folder.
- Visual checks in the headless sway of the shared rules (section 17), with isolated
  `XDG_CONFIG_HOME` and `XDG_STATE_HOME`. The window opens at 1280x760 in the 1600x1000 output;
  the side bar entries are at x 230.
- Windows: cross-build with `cargo xwin build --release --target x86_64-pc-windows-msvc -p
  latexo-app` (an isolated rustup, see the Windows VM notes in Postino's memory) into
  `target/xwin`, copy `latexo.exe` to the `windows11` VM and run it there.
- Debug hooks, read once at startup: `LATEXO_LOG=<level>`, `LATEXO_OPEN=<screen>` (`overview`,
  `processes`, `cpu`, `cores`, `gpu`, `memory`, `disk`, `network`, `temperatures`, `fans`,
  `energy`, `alerts`, `settings`, `about`), `LATEXO_UPDATE_URL` (updater manifest).

## Working rules

- Git remote `origin` is `tanisperez/latexo`. Commits are fine; never `git push` unless the user
  says so explicitly. Commit messages, issues and pull requests in English.
- Work is tracked in the GitHub milestone of each version (`gh issue list --milestone 0.1.0`).
  Complex issues go on a branch.
- Keep changes small and traceable to the request. Do not refactor unrelated code.
- Run `make format`, `make lint` and `make test` before considering a task done.
- GPUI evolves quickly. Read the pinned `gpui-kit` source in `~/.cargo/registry/src/` instead of
  relying on memory of its API. `use gpui_kit::*` brings gpui's `test` macro into scope: test
  modules of view files import what they test by name instead of `use super::*`.
- Writing style: never use the em dash in code, comments, docs or translations.

## Commands

See `make help`. Main targets: `run`, `build`, `release`, `check`, `lint`, `format`, `test`,
`dist`, `install`.
