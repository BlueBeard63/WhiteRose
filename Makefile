# WhiteRose. Run `make check` before every commit; CI runs the same targets.

GO       ?= go
CARGO    ?= cargo
DAEMON   := daemon
WM       := compositor
FUZZTIME ?= 20s

.PHONY: check go-check rust-check shell-check fmt vet race fuzz cover rust-fmt clippy rust-test build run-nested clean

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

clean:
	rm -rf build $(DAEMON)/coverage.out $(DAEMON)/coverage.html
	cd $(WM) && $(CARGO) clean
