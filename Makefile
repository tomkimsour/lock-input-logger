PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
UNITDIR ?= /etc/systemd/system
LOCK_LOGGER_USER ?= $(or $(SUDO_USER),$(USER))
SUDO ?= $(if $(filter 0,$(shell id -u)),,sudo)
SYSTEMCTL ?= systemctl

BIN := target/release/lock-input-logger
UNIT := target/lock-input-logger.service

.PHONY: all build install uninstall clean

all: build

build: $(BIN)

$(BIN): Cargo.toml Cargo.lock $(wildcard src/*.rs)
	cargo build --release --locked

install: $(BIN)
	@test -n "$(LOCK_LOGGER_USER)" -a "$(LOCK_LOGGER_USER)" != root || { echo "Set LOCK_LOGGER_USER to the login to watch, e.g. make install LOCK_LOGGER_USER=alice" >&2; exit 1; }
	sed -e 's|@BINDIR@|$(BINDIR)|g' -e 's|@USER@|$(LOCK_LOGGER_USER)|g' lock-input-logger.service > $(UNIT)
	$(SUDO) install -Dm755 $(BIN) $(BINDIR)/lock-input-logger
	$(SUDO) install -Dm644 $(UNIT) $(UNITDIR)/lock-input-logger.service
	$(SUDO) $(SYSTEMCTL) daemon-reload
	$(SUDO) $(SYSTEMCTL) enable lock-input-logger.service
	$(SUDO) $(SYSTEMCTL) restart lock-input-logger.service

uninstall:
	-$(SUDO) $(SYSTEMCTL) disable --now lock-input-logger.service
	$(SUDO) rm -f $(BINDIR)/lock-input-logger $(UNITDIR)/lock-input-logger.service
	$(SUDO) $(SYSTEMCTL) daemon-reload

clean:
	cargo clean
