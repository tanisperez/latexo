# Architecture

Latexo is a Cargo workspace of four crates. Only `latexo-app` knows gpui.

```
latexo-app ──> latexo-sensors ──> latexo-core
    ├────────> latexo-core
    ├────────> latexo-cli
    └────────> telaio-ui, telaio-app, telaio-update
```

## latexo-core

The domain model, plain data and plain functions, no IO:

- `sample`: `Sample`, one reading of the whole machine (CPU and each logical processor, memory,
  disk throughput, volumes, network and interfaces, temperature sensors, fans, power, GPUs,
  processes), and `SystemInfo`, what does not change while the app runs. A value the platform
  cannot read is `None` or an empty list, never zero.
- `history`: `History`, every series (`SeriesKey`) aligned on one ring of times. Entry `i` of
  every series was read at `times[i]`; a series that appears later is padded with `NaN`, and a
  sample that misses it adds a `NaN`, drawn as a gap. `downsample` reduces a series to the
  points a chart has room for.
- `process`: `ProcessInfo`, `ProcessQuery` (sort by any column, filter by owner, search by name
  or PID) and `tree` (a process and its descendants).
- `alerts`: `Rule` (metric, threshold, above or below, duration, action) and `Watcher`, which
  fires a rule once when its condition has held for its duration and rearms it when it clears.
- `energy`: `EnergyAccount`, watt hours today and this month, reset when the local day or month
  changes. Gaps over a minute (a suspended laptop) are not counted.
- `minutes`: the minute history of the long ranges: `Aggregator` turns samples into the average
  of each minute, `fill_gap` marks the minutes Latexo was not running, and `to_text` and
  `from_text` write and read `history.tsv` (a header with the series names, then one line per
  minute, tab separated, a gap as an empty field).
- `units`: the unit and decimals of sizes and rates, temperatures in °C or °F, durations.

## latexo-sensors

`Sampler::new` opens the platform handles and takes the first readings; `Sampler::sample`
returns one `Sample` whose rates cover the time since the previous call.

| Reading | Linux | Windows |
| --- | --- | --- |
| CPU, each logical processor, clock | sysinfo | sysinfo |
| User and kernel split | `/proc/stat` | not read |
| Load average | sysinfo | none |
| Memory, swap | sysinfo | sysinfo (page file) |
| Volumes, disk throughput | sysinfo, each device counted once | sysinfo |
| Network, interfaces | sysinfo | sysinfo |
| Temperatures | sysinfo (`hwmon`), grouped by `classify` | sysinfo (WMI thermal zones, needs administrator) |
| Temperature of each core | `coretemp` matched with `topology/core_id` | none |
| Fans | `hwmon` `fan*_input` | none without a board driver |
| Package power | `powercap` `intel-rapl:0` (root only on most systems) | none without a driver |
| Battery | `power_supply` (charge, power, mains) | `starship-battery` |
| GPUs, usage per process | sysinfo (NVML, Vulkan, `gpu_busy_percent`) | sysinfo (performance counters) |
| Processes | sysinfo; kernel threads left out; names cut at 15 characters take the executable's name | sysinfo |

`end_processes` ends processes by PID: `SIGTERM` on Linux, `TerminateProcess` on Windows.

The `probe` example prints what a machine reports: `cargo run -p latexo-sensors --example probe`.

## latexo-app

- `main.rs`: the command line, the settings, the logger, the language, the theme and the window,
  as ted.
- `views/root.rs`: `AppView` owns the settings and the `Monitor`. It starts the **sampling
  thread** (`latexo-sampler`): the thread opens the `Sampler`, sends the `SystemInfo`, then reads
  the machine, sends the `Sample` through a bounded `async-channel` and sleeps for the rest of the
  interval. The interval is an `AtomicU64` that Settings updates. A task on the window waits on
  the channel and pushes each sample into the `Monitor` (history, energy, alerts), then
  notifies: an idle Latexo redraws once per sample and does nothing else. The sampler stays on
  its thread because sysinfo's GPU handles are not `Send`.
- `views/chart.rs`: the chart widget, painted with a `canvas`. One series or two stacked, in four
  styles (line with its area, bars, steps, dots). Values are bucketed at paint time to at most
  one point per two pixels, the newest at the right edge.
- One module per screen: `overview`, `processes` (a `uniform_list`, only the visible rows are
  built), `resources` (CPU, Memory, Disk, Network, Temperatures, Energy share one layout),
  `cores`, `gpu` (GPU and Fans), `alerts`, `settings`, `about`.
- `state/`: settings, monitor, formatting in the active language, navigation, autostart, the
  updater gate and the tests of the translations.

## Files Latexo writes

| File | Where | What |
| --- | --- | --- |
| `settings.toml` | config folder (`~/.config/latexo`, `%APPDATA%\latexo`) | every setting and the alert rules |
| `energy.toml` | config folder | watt hours today and this month |
| `history.tsv` | config folder | the average of every series per minute, for 24 h or 7 days (none with "1 h") |
| `window.toml` | config folder | the window size and whether it was maximized |
| `latexo.log` | `~/.local/state/latexo`, `%LOCALAPPDATA%\Latexo\logs` | the log, rotated at 5 MB, 10 files |
| `codes.tanis.latexo.desktop` | `~/.config/autostart` (Linux) | only with "Start when I log in" |
| `Latexo` value | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (Windows) | only with "Start with Windows" |
| the exported CSV | where the user saves it | the history of the last hour |
