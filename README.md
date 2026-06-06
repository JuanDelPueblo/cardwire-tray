# Cardwire Tray

<img width="755" height="298" alt="image" src="https://github.com/user-attachments/assets/39422cb1-d18a-47b5-a7f4-06de6fb6e38d" />

### An universal tray applet for [Cardwire](https://github.com/OpenGamingCollective/cardwire) GPU manager.

This applet implements all current Cardwire features including mode switching between integrated, hybrid, and manual mode. It also supports manual GPU blocking while in manual mode. Hovering over the tray icon shows all info about your GPUs such as their name, power state, and block status. Configs for Cardwire are also included such as for battery auto switch, auto apply GPU state, and experimental Nvidia block.

## Install

Install the latest version using [my Flatpak repo](https://juandelpueblo.github.io/cardwire-tray/flatpak/me.edyan.cardwiretray.flatpakref). If you don't have Flatpak, build and compile using make.

Dependencies: `rust`, `cargo`, `cardwire` v0.9.0 or later 

```bash
make
sudo make install
```
