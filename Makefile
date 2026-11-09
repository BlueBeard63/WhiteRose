# WhiteRose. Run `make check` before every commit; CI runs the same targets.

GO       ?= go
CARGO    ?= cargo
DAEMON   := daemon
WM       := compositor
FUZZTIME ?= 20s

.PHONY: check go-check rust-check shell-check fmt vet race fuzz cover rust-fmt clippy rust-test build run-nested run-tty package package-deb package-rpm clean

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
	cd $(DAEMON) && $(GO) test -run='^$$' -fuzz=FuzzParse -fuzztime=$(FUZZTIME) ./internal/edid
	cd $(DAEMON) && $(GO) test -run='^$$' -fuzz=FuzzParse -fuzztime=$(FUZZTIME) ./internal/uevent
	cd $(DAEMON) && $(GO) test -run='^$$' -fuzz=FuzzParse -fuzztime=$(FUZZTIME) ./internal/ical

cover:
	cd $(DAEMON) && $(GO) test -coverprofile=coverage.out ./... && $(GO) tool cover -html=coverage.out -o coverage.html

# ---- Rust compositor -------------------------------------------------------
rust-check: rust-fmt clippy rust-test

rust-fmt:
	cd $(WM) && $(CARGO) fmt --check

clippy:
	cd $(WM) && $(CARGO) clippy --all-targets --locked -- -D warnings

rust-test:
	cd $(WM) && $(CARGO) test --locked

# ---- Shell (QuickShell QML) -----------------------------------------------
shell-check:         ## pure JS logic: calculator, search ranking, recent files
	node shell/tests/run.mjs

# ---- Build / run -----------------------------------------------------------
build:
	cd $(DAEMON) && CGO_ENABLED=0 $(GO) build -trimpath -ldflags='-s -w' -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && $(CARGO) build --release --locked && cp target/release/whiterose-wm ../build/

# Its own D-Bus session bus, like a real WhiteRose login: WhiteRose is then the
# notification server (GNOME keeps that name on the host bus), and apps started
# inside notify it. System services (network, Bluetooth, power) are shared.
run-nested:          ## WhiteRose in a window on your current desktop: bar + a terminal
	cd $(DAEMON) && $(GO) build -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && PATH="$(CURDIR)/build:$$PATH" dbus-run-session -- $(CARGO) run --release -- --daemon "whiterosed serve" --shell "qs -p $(CURDIR)/shell" -c "ptyxis --standalone"

run-tty:             ## WhiteRose on the real screens: log in on a console (Ctrl+Alt+F3) first
	cd $(DAEMON) && $(GO) build -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && $(CARGO) build --release
	@if [ -n "$$WAYLAND_DISPLAY$$DISPLAY" ]; then echo "run-tty takes over the screens, so it must start from a text console: press Ctrl+Alt+F3, log in, then run make run-tty there (make run-nested runs it in a window here)."; exit 1; fi
	@echo "Starting on this console; Ctrl+Alt+F1/F2 gets you back to your desktop. Log: build/whiterose-tty.log"
	PATH="$(CURDIR)/build:$$PATH" dbus-run-session -- $(WM)/target/release/whiterose-wm --backend drm --daemon "whiterosed serve" --shell "qs -p $(CURDIR)/shell" -c "ptyxis --standalone" > build/whiterose-tty.log 2>&1

# ---- Packages ----------------------------------------------------------------
# One .deb (Ubuntu 26.04) and one .rpm (Fedora), each built in its distro's
# container against that distro's Qt and libraries. Output: build/packages/.
VERSION ?= 0.1.0
package: package-deb package-rpm

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
