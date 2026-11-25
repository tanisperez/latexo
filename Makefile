.DEFAULT_GOAL := help

.PHONY: help run build release dist install uninstall check lint lint-fix format format-check test clean docs site

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "  run           Build and run the app (debug)"
	@echo "  build         Build in debug mode"
	@echo "  release       Build an optimized release binary"
	@echo "  dist          Build the release packages for this OS into target/dist"
	@echo "  install       Linux: build and install for this user under ~/.local (PREFIX=...)"
	@echo "  uninstall     Linux: remove what make install put under PREFIX"
	@echo "  check         Fast type-check without producing a binary"
	@echo "  lint          Run Clippy (warnings are errors)"
	@echo "  lint-fix      Run Clippy and apply automatic fixes"
	@echo "  format        Format all Rust code with rustfmt"
	@echo "  format-check  Check formatting without modifying files"
	@echo "  test          Run all tests"
	@echo "  clean         Remove build artifacts"
	@echo "  docs          Build the user documentation (site/docs-src into site/docs)"
	@echo "  site          Build the documentation and serve the website on http://localhost:8000"

run:
	cargo run

build:
	cargo build

release:
	cargo build --release

# The workspace version from Cargo.toml, for the package names.
VERSION := $(shell sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)

# Linux needs nfpm on PATH. On Windows, compile packaging/windows/latexo.iss with Inno Setup (see
# its README). Latexo has no macOS build.
dist: release
	@case "$$(uname -s)" in \
	Linux) packaging/linux/package.sh $(VERSION) target/release/latexo target/dist ;; \
	*) echo "make dist supports Linux; see packaging/windows/README.md"; exit 1 ;; \
	esac

# Linux only: the binary, the desktop entry and the icons. The desktop entry is what gives the
# window its icon on Wayland (matched by the app id). Its Exec line gets the absolute path, so the
# menu entry works even when PREFIX/bin is not on the session's PATH.
PREFIX ?= $(HOME)/.local
APP_ID := codes.tanis.latexo
ICON_SIZES := 16 24 32 48 64 128 256 512
ICONS_DIR = $(PREFIX)/share/icons/hicolor

install: release
	@[ "$$(uname -s)" = Linux ] || { echo "make install is for Linux, see packaging/ for Windows"; exit 1; }
	install -Dm755 target/release/latexo $(PREFIX)/bin/latexo
	sed 's|^Exec=latexo|Exec=$(PREFIX)/bin/latexo|' packaging/linux/$(APP_ID).desktop \
		| install -Dm644 /dev/stdin $(PREFIX)/share/applications/$(APP_ID).desktop
	install -Dm644 packaging/icons/latexo.svg $(ICONS_DIR)/scalable/apps/$(APP_ID).svg
	for size in $(ICON_SIZES); do \
		install -Dm644 packaging/icons/png/latexo-$$size.png \
			$(ICONS_DIR)/$${size}x$${size}/apps/$(APP_ID).png; \
	done
	-update-desktop-database -q $(PREFIX)/share/applications 2>/dev/null

uninstall:
	rm -f $(PREFIX)/bin/latexo $(PREFIX)/share/applications/$(APP_ID).desktop
	rm -f $(ICONS_DIR)/scalable/apps/$(APP_ID).svg \
		$(foreach size,$(ICON_SIZES),$(ICONS_DIR)/$(size)x$(size)/apps/$(APP_ID).png)
	-update-desktop-database -q $(PREFIX)/share/applications 2>/dev/null

check:
	cargo check --all-targets

lint:
	cargo clippy --all-targets -- -D warnings

lint-fix:
	cargo clippy --all-targets --fix --allow-dirty --allow-staged

format:
	cargo fmt --all

format-check:
	cargo fmt --all -- --check

test:
	cargo test --all-targets

clean:
	cargo clean

PYTHON ?= python3

# site/docs is generated, never edited by hand.
docs:
	$(PYTHON) site/docs-src/build.py

site: docs
	$(PYTHON) -m http.server -d site 8000
