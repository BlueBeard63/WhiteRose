# WhiteRose. Run `make check` before every commit; CI runs the same targets.

GO       ?= go
CARGO    ?= cargo
DAEMON   := daemon
WM       := compositor
FUZZTIME ?= 20s

.PHONY: check go-check rust-check fmt vet race fuzz cover rust-fmt clippy rust-test build run-nested clean

check: go-check rust-check

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

# ---- Build / run -----------------------------------------------------------
build:
	cd $(DAEMON) && CGO_ENABLED=0 $(GO) build -trimpath -ldflags='-s -w' -o ../build/whiterosed ./cmd/whiterosed
	cd $(WM) && $(CARGO) build --release --locked && cp target/release/whiterose-wm ../build/

run-nested:          ## WhiteRose in a window on your current desktop: bar + a terminal
	cd $(WM) && $(CARGO) run --release -- --shell "qs -p $(CURDIR)/shell" -c "ptyxis --standalone"

clean:
	rm -rf build $(DAEMON)/coverage.out $(DAEMON)/coverage.html
	cd $(WM) && $(CARGO) clean
