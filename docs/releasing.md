# Releasing Latexo

Publishing a version means pushing a tag. The `Release` workflow
(`.github/workflows/release.yml`, ted's without macOS) builds Latexo for Linux and Windows,
attaches the packages to the GitHub release of that tag and publishes it, with `latest.json`, the
manifest of the in-app updater. To try an update between pre-releases, start a release build with
`LATEXO_UPDATE_URL` set to the address of another `latest.json`.

## Versions and tags

- The version lives in one place: `version` under `[workspace.package]` in `Cargo.toml`.
  Versions follow SemVer.
- Between two releases the version is the next one with the `-dev` suffix (`0.2.0-dev`). Right
  after a release, bump `version` to the next `X.Y.Z-dev`.
- A development build shows the short commit next to the version in Settings, About, in
  `latexo --version` (`Latexo 0.2.0-dev (4e74e2d)`) and in the startup log line. It comes from
  `latexo-cli/build.rs`. Releases and release candidates show the bare version.
- The tag is `v` plus that version: `v0.1.0`. The workflow fails if they differ, so set the final
  version (drop `-dev`) before tagging. A tag with a pre-release suffix (`v0.1.0-rc.1`) is
  published as a GitHub pre-release.

## Steps

The work of a version is tracked by its GitHub milestone (`gh issue list --milestone X.Y.Z`).

1. Check that every issue of the milestone is closed and `make format`, `make lint` and
   `make test` pass on `main`.
2. Optionally, run the workflow by hand on `main` first (`gh workflow run release.yml`): a dry
   run builds every package and keeps them as the run's artifacts.
3. Write `release-notes/X.Y.Z.md` (English Markdown: what changes and which milestone issues went
   in). Set `version` in `Cargo.toml` to the release, run `cargo check` so `Cargo.lock` follows,
   and commit.
4. Tag and push (ask before pushing): `git tag v0.1.0 && git push origin main v0.1.0`.
5. Create the release as a draft:
   `gh release create v0.1.0 --draft --verify-tag --title v0.1.0 --notes-file release-notes/0.1.0.md`.
6. Wait for the `Release` workflow (`gh run watch`); it uploads every asset and publishes the
   draft.
7. Bump `version` to the next `X.Y.Z-dev` and commit.

## Packages

Targets: Linux x86_64 (built on Ubuntu 22.04, glibc 2.35) and Windows x86_64. No macOS.

| Asset | Content |
| --- | --- |
| `latexo-X.Y.Z-linux-x86_64.tar.gz` | `bin/latexo`, the desktop entry and the icons in a `/usr`-like layout |
| `latexo_X.Y.Z_amd64.deb`, `latexo-X.Y.Z-1.x86_64.rpm` | built with `nfpm` from `packaging/linux/nfpm.yaml` |
| `latexo-X.Y.Z-1-x86_64.pkg.tar.zst` | Arch package, written by `package.sh` with `bsdtar` and `zstd` |
| `latexo-X.Y.Z-x86_64.AppImage` | the same files in an AppImage |
| `latexo-X.Y.Z-windows-x86_64-setup.exe` | Inno Setup, **per-user** install, no administrator rights |
| `latexo-X.Y.Z-windows-x86_64-portable.zip` | `latexo.exe`, `LICENSE` and `NOTICE` |
| `latest.json`, `SHA256SUMS` | the updater manifest and the checksums |

Locally, on Linux, with `nfpm`, `bsdtar` and `zstd` on the `PATH`: `make dist`.

## Desktop integration

- **Icons:** one master, `packaging/icons/latexo.svg`; the PNGs and `latexo.ico` are generated
  from it and committed (`packaging/icons/README.md`).
- **Linux:** `codes.tanis.latexo.desktop` (the Wayland `app_id` and X11 `WM_CLASS`), in the
  System and Monitor categories.
- **Windows:** a Start menu entry with the AppUserModelID `codes.tanis.latexo` and an optional
  desktop icon. The uninstaller removes the "Start with Windows" value.
- **Unsigned for now:** SmartScreen warns on the first launch.
