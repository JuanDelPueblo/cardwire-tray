PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
APPDIR ?= $(DATADIR)/applications
ICONDIR ?= $(DATADIR)/icons/hicolor/scalable/apps

TARGET = target/release/cardwire-tray

.PHONY: all build run clean install uninstall rpm

all: build

build:
	cargo build --release

run:
	cargo run

clean:
	cargo clean

install:
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 $(TARGET) $(DESTDIR)$(BINDIR)/cardwire-tray
	install -d $(DESTDIR)$(APPDIR)
	install -m 644 me.edyan.cardwiretray.desktop $(DESTDIR)$(APPDIR)/me.edyan.cardwiretray.desktop
	install -d $(DESTDIR)$(ICONDIR)
	install -m 644 icons/gpu.svg $(DESTDIR)$(ICONDIR)/me.edyan.cardwiretray.svg
	install -m 644 icons/gpu.svg $(DESTDIR)$(ICONDIR)/me.edyan.cardwiretray-gpu.svg
	install -m 644 icons/integrated.svg $(DESTDIR)$(ICONDIR)/me.edyan.cardwiretray-integrated.svg
	install -m 644 icons/hybrid.svg $(DESTDIR)$(ICONDIR)/me.edyan.cardwiretray-hybrid.svg
	install -m 644 icons/manual.svg $(DESTDIR)$(ICONDIR)/me.edyan.cardwiretray-manual.svg
	install -m 644 icons/smart.svg $(DESTDIR)$(ICONDIR)/me.edyan.cardwiretray-smart.svg

uninstall:
	rm -f $(BINDIR)/cardwire-tray
	rm -f $(APPDIR)/me.edyan.cardwiretray.desktop
	rm -f $(ICONDIR)/me.edyan.cardwiretray.svg
	rm -f $(ICONDIR)/me.edyan.cardwiretray-gpu.svg
	rm -f $(ICONDIR)/me.edyan.cardwiretray-integrated.svg
	rm -f $(ICONDIR)/me.edyan.cardwiretray-hybrid.svg
	rm -f $(ICONDIR)/me.edyan.cardwiretray-manual.svg
	rm -f $(ICONDIR)/me.edyan.cardwiretray-smart.svg
