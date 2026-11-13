# WhiteRose. Run `make check` before every commit; CI runs the same targets.

GO       ?= go
CARGO    ?= cargo
DAEMON   := daemon
WM       := compositor
# Fuzzing runs a fixed number of inputs, not a time: a time limit can end
# while Go is still handling a new input and report "context deadline
# exceeded" as a failure. Each count is about 20 s on CI's 4 cores (much
# less on a bigger machine). FUZZTIME=30s (or 5000000x) sets all three.
FUZZ_EDID   ?= 2000000x
FUZZ_UEVENT ?= 2000000x
FUZZ_ICAL   ?= 1000000x

.PHONY: release-key release release-notes changelog ui-test shots dev-apps dev-apps-remove check go-check rust-check shell-check fmt vet race fuzz cover rust-fmt clippy rust-test build run-nested run-tty package package-deb package-rpm clean

check: go-check rust-check shell-check

# ---- Go daemon -------------------------------------------------------------
go-check: fmt vet race fuzz

fmt:
	@out="$$(gofmt -l $(DAEMON))"; if [ -n "$$out" ]; then echo "gofmt needed:"; echo "$$out"; exit 1; fi

vet:
	cd $(DAEMON) && $(GO) vet ./...

race:
	cd $(DAEMON) && $(GO) test -race -count=1 ./...

fuzz:
	cd $(DAEMON) && $(GO) test -run='^$$' -fuzz=FuzzParse -fuzztime=$(or $(FUZZTIME),$(FUZZ_EDID)) ./internal/edid
	cd $(DAEMON) && $(GO) test -run='^$$' -fuzz=FuzzParse -fuzztime=$(or $(FUZZTIME),$(FUZZ_UEVENT)) ./internal/uevent
	cd $(DAEMON) && $(GO) test -run='^$$' -fuzz=FuzzParse -fuzztime=$(or $(FUZZTIME),$(FUZZ_ICAL)) ./internal/ical

cover:
	cd $(DAEMON) && $(GO) test -coverprofile=coverage.out ./... && $(GO) tool cover -html=coverage.out -o coverage.html

# ---- Rust compositor -------------------------------------------------------
rust-check: rust-fmt clippy rust-test

rust-fmt:
	cd $(WM) && $(CARGO) fmt --check
	cd apps && $(CARGO) fmt --check

clippy:
	cd $(WM) && $(CARGO) clippy --all-targets --locked -- -D warnings
	cd apps && $(CARGO) clippy --all-targets --locked -- -D warnings

rust-test:
	cd $(WM) && $(CARGO) test --locked
	cd apps && $(CARGO) test --locked

# ---- Shell (QuickShell QML) -----------------------------------------------
shell-check:         ## pure JS logic: calculator, search ranking, recent files
	node shell/tests/run.mjs

# ---- Build / run -----------------------------------------------------------
build:
	cd $(DAEMON) && CGO_ENABLED=0 $(GO) build -trimpath -ldflags='-s -w' -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && $(CARGO) build --release --locked && cp target/release/whiterose-wm ../build/

# Click-and-check tests (daemon/uitest): each runs a nested WhiteRose in a
# session of its own (its own HOME, D-Bus, accessibility bus, display),
# finds things by their accessible names, clicks and types with the real
# pointer and keyboard, and checks what shows. Inside a Wayland desktop the
# sessions are nested windows; without one (or with WR_UITEST_HEADLESS=1, as
# CI does) they use the compositor's headless backend. UITEST=Name runs only
# tests matching Name.
shots:               ## Screenshots for review: the bar's icons and every Settings page at tile sizes (build/shots)
	cd $(DAEMON) && $(GO) build -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && $(CARGO) build --release --locked
	cd apps && $(CARGO) build --locked
	rm -rf build/shots
	cd $(DAEMON) && WHITEROSE_SHOTS=$(CURDIR)/build/shots $(GO) test -tags 'uitest shots' -count=1 -timeout 20m -run TestShots ./uitest/

ui-test:             ## Run the click-and-check UI tests
	cd $(DAEMON) && $(GO) build -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && $(CARGO) build --release --locked
	cd apps && $(CARGO) build --locked
	cd $(DAEMON) && $(GO) test -tags uitest -count=1 $(if $(UITEST),-run '$(UITEST)') ./uitest/

# Its own D-Bus session bus, like a real WhiteRose login: WhiteRose is then the
# notification server (GNOME keeps that name on the host bus), and apps started
# inside notify it. System services (network, Bluetooth, power) are shared.
run-nested:          ## WhiteRose in a window on your current desktop: bar + a terminal
	cd $(DAEMON) && $(GO) build -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && PATH="$(CURDIR)/build:$$PATH" WHITEROSE_FILES="$(CURDIR)/apps/target/release/whiterose-files" dbus-run-session -- $(CARGO) run --release -- --daemon "whiterosed serve" --shell "qs -p $(CURDIR)/shell" -c "ptyxis --standalone"

# Settings, Notepad and Files in the launcher, with their icons, running
# straight from this checkout (your ~/.local only; dev-apps-remove undoes it).
# An installed package's entries are shadowed by these until then.
dev-apps:            ## Settings, Notepad and Files in your launcher, run from this checkout
	cd apps && $(CARGO) build --release --locked
	mkdir -p $(HOME)/.local/share/applications $(HOME)/.local/share/icons
	for a in notepad files; do \
		sed "s|^Exec=whiterose-|Exec=$(CURDIR)/apps/target/release/whiterose-|" packaging/files/whiterose-$$a.desktop \
			> $(HOME)/.local/share/applications/whiterose-$$a.desktop; \
	done
	sed "s|^Exec=whiterose-settings|Exec=$(CURDIR)/shell/bin/whiterose-settings|" packaging/files/whiterose-settings.desktop \
		> $(HOME)/.local/share/applications/whiterose-settings.desktop
	cp -r packaging/icons/hicolor $(HOME)/.local/share/icons/
	# (Files opens Settings › Files with it.)
	mkdir -p $(HOME)/.local/bin && ln -sf $(CURDIR)/shell/bin/whiterose-settings $(HOME)/.local/bin/whiterose-settings
	-update-desktop-database -q $(HOME)/.local/share/applications 2>/dev/null

dev-apps-remove:     ## Takes dev-apps' launcher entries and icons out again
	rm -f $(HOME)/.local/share/applications/whiterose-notepad.desktop $(HOME)/.local/share/applications/whiterose-files.desktop \
		$(HOME)/.local/share/applications/whiterose-settings.desktop
	rm -f $(HOME)/.local/bin/whiterose-settings
	rm -f $(HOME)/.local/share/icons/hicolor/*/apps/whiterose-files.png $(HOME)/.local/share/icons/hicolor/*/apps/whiterose-notepad.png \
		$(HOME)/.local/share/icons/hicolor/*/apps/whiterose-settings.png

run-tty:             ## WhiteRose on the real screens: log in on a console (Ctrl+Alt+F3) first
	cd $(DAEMON) && $(GO) build -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && $(CARGO) build --release
	@if [ -n "$$WAYLAND_DISPLAY$$DISPLAY" ]; then echo "run-tty takes over the screens, so it must start from a text console: press Ctrl+Alt+F3, log in, then run make run-tty there (make run-nested runs it in a window here)."; exit 1; fi
	@echo "Starting on this console; Ctrl+Alt+F1/F2 gets you back to your desktop. Log: build/whiterose-tty.log"
	PATH="$(CURDIR)/build:$$PATH" WHITEROSE_FILES="$(CURDIR)/apps/target/release/whiterose-files" dbus-run-session -- $(WM)/target/release/whiterose-wm --backend drm --daemon "whiterosed serve" --shell "qs -p $(CURDIR)/shell" -c "ptyxis --standalone" > build/whiterose-tty.log 2>&1

# ---- Packages ----------------------------------------------------------------
# One .deb (Ubuntu 26.04) and one .rpm (Fedora), each built in its distro's
# container against that distro's Qt and libraries. Output: build/packages/.
# The version comes from project.yaml (VERSION=… overrides it).
PROJECT  := project.yaml
project   = cd $(DAEMON) && $(GO) run ./cmd/whiterosed project $(CURDIR)/$(PROJECT)
VERSION  ?= $(shell $(project) version)
package: package-deb package-rpm

# ---- releases (updates) ---------------------------------------------------------
# The release key signs every release; its public half (packaging/release.pub)
# ships in the packages, so WhiteRose only installs updates signed with it.
# The private half stays in ~/.config/whiterose-release (back it up).
release-key:         ## Make the release signing key (once)
	cd $(DAEMON) && $(GO) run ./cmd/whiterosed release-key $(CURDIR)/packaging/release.pub

# `make release`: project.yaml's version, both packages, signed, checked
# with the installer (packaging/install.sh, attached as install.sh), published
# as a GitHub release (a pre-release for versions like 0.6.0-alpha.12),
# with that version's entry in project.yaml as the notes (NOTES=file to
# use your own). Needs `gh` logged in. Add the version's entry first:
# `make release-notes` shows what will be published.
RELEASE_REPO ?= $(shell $(project) repo)
release:             ## Build, sign and publish project.yaml's version as a GitHub release
	@test -n "$(RELEASE_REPO)" || { echo "no repo in $(PROJECT) (repo: owner/name)"; exit 1; }
	@test -f packaging/release.pub || { echo "no release key yet: make release-key"; exit 1; }
	@$(project) check
	@gh release view v$(VERSION) --repo $(RELEASE_REPO) >/dev/null 2>&1 && { echo "v$(VERSION) is already released: change the version in $(PROJECT)"; exit 1; } || true
	mkdir -p build
	@if [ -n "$$NOTES" ]; then cp "$$NOTES" build/release-notes.md; else $(project) notes $(VERSION) > $(CURDIR)/build/release-notes.md; fi
	rm -rf build/packages/* && $(MAKE) package VERSION=$(VERSION)
	cd $(DAEMON) && $(GO) run ./cmd/whiterosed sign-release $(CURDIR)/build/packages $(CURDIR)/packaging/release.pub
	# The one-command installer, for this version, repository and key.
	sed -e 's|@VERSION@|$(VERSION)|g' -e 's|@REPO@|$(RELEASE_REPO)|g' -e "s|@PUBKEY@|$$(tr -d '[:space:]' < packaging/release.pub)|g" \
		packaging/install.sh > build/packages/install.sh
	cd build/packages && sh install.sh --check
	gh release create v$(VERSION) --repo $(RELEASE_REPO) --title "WhiteRose $(VERSION)" \
		$$($(project) prerelease && echo --prerelease) \
		--notes-file build/release-notes.md build/packages/*.deb build/packages/*.rpm build/packages/SHA256SUMS build/packages/SHA256SUMS.sig build/packages/install.sh

release-notes:       ## Show the release notes `make release` will publish
	@$(project) notes $(VERSION)

changelog:           ## Write CHANGELOG.md from project.yaml
	@$(project) changelog > $(CURDIR)/CHANGELOG.md && echo "CHANGELOG.md: written from $(PROJECT)"

package-deb:
	docker build -q -t whiterose-build-ubuntu -f packaging/Dockerfile.ubuntu packaging >/dev/null
	mkdir -p build/packages
	docker run --rm -v "$(CURDIR)":/src:ro -v "$(CURDIR)/build/packages":/out \
		-v whiterose-ubuntu-work:/work -v whiterose-cargo:/root/.cargo/registry -v whiterose-gomod:/root/go/pkg/mod \
		whiterose-build-ubuntu sh /src/packaging/build.sh deb $(VERSION)

package-rpm:
	docker build -q -t whiterose-build-fedora -f packaging/Dockerfile.fedora packaging >/dev/null
	mkdir -p build/packages
	docker run --rm -v "$(CURDIR)":/src:ro -v "$(CURDIR)/build/packages":/out \
		-v whiterose-fedora-work:/work -v whiterose-cargo:/root/.cargo/registry -v whiterose-gomod:/root/go/pkg/mod \
		whiterose-build-fedora sh /src/packaging/build.sh rpm $(VERSION)

clean:
	rm -rf build $(DAEMON)/coverage.out $(DAEMON)/coverage.html
	cd $(WM) && $(CARGO) clean
