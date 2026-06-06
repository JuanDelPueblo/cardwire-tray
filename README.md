# Cardwire Tray

<img width="846" height="341" alt="image" src="https://github.com/user-attachments/assets/4b083b1d-dea3-473b-b3ac-d0c9def6408a" />

### An universal tray applet for [Cardwire](https://github.com/OpenGamingCollective/cardwire) GPU manager.

This applet implements all current Cardwire features including mode switching between integrated, hybrid, manual, and smart mode. It also supports manual GPU blocking while in manual mode. Hovering over the tray icon shows all info about your GPUs such as their name, power state, and block status.

## Install

Install the latest version using [my Flatpak repo](https://juandelpueblo.github.io/cardwire-tray/flatpak/me.edyan.cardwiretray.flatpakref). If you don't have Flatpak, build and compile using make.

Dependencies: `rust`, `cargo`, `cardwire` v0.9.0 or later 

```bash
make
sudo make install
```
