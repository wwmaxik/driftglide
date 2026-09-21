# DriftGlide Makefile
# Installation and build automation

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DESTDIR ?=
CARGO ?= cargo
TARGET = target/release/driftglide

.PHONY: all build install uninstall clean help

all: build

## build: Build release binary using cargo
build:
	$(CARGO) build --release

## install: Install binary to $(DESTDIR)$(BINDIR)
install: build
	install -d "$(DESTDIR)$(BINDIR)"
	install -m 755 "$(TARGET)" "$(DESTDIR)$(BINDIR)/driftglide"
	@echo ""
	@echo "DriftGlide installed successfully to $(DESTDIR)$(BINDIR)/driftglide"
	@echo ""
	@echo "To enable autostart in driftwm, add to your ~/.config/driftwm/config.toml:"
	@echo '    autostart = ["driftglide"]'
	@echo ""

## uninstall: Remove installed binary
uninstall:
	rm -f "$(DESTDIR)$(BINDIR)/driftglide"
	@echo "DriftGlide uninstalled from $(DESTDIR)$(BINDIR)"

## clean: Remove cargo build artifacts
clean:
	$(CARGO) clean

## help: Display available targets
help:
	@echo "DriftGlide Build & Install Targets:"
	@echo "  make build      - Build release binary (default)"
	@echo "  make install    - Install to $(BINDIR) (requires sudo if default prefix)"
	@echo "  make uninstall  - Remove installed files"
	@echo "  make clean      - Clean build artifacts"
