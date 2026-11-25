# Linux packaging

| File | Purpose |
|---|---|
| `codes.tanis.latexo.desktop` | Desktop entry (`Exec=latexo`) |
| `nfpm.yaml` | nfpm config shared by the deb and the rpm |
| `package.sh` | Builds the tarball, the deb, the rpm and the Arch package |
| `appimage.sh` | Builds the AppImage |

The packages are named `latexo`; the binary is `/usr/bin/latexo`. The signed APT and DNF
repositories and the install script of Postino and ted are not set up for Latexo yet.
