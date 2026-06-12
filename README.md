# Cardwire Tray

<img width="846" height="341" alt="image" src="https://github.com/user-attachments/assets/4b083b1d-dea3-473b-b3ac-d0c9def6408a" />

### A universal GUI frontend and tray applet for [Cardwire](https://github.com/OpenGamingCollective/cardwire) GPU manager.

This application implements all current Cardwire features including mode switching (between integrated, hybrid, manual, and smart modes) and manual GPU blocking. It consists of:
- A tray applet providing quick mode-toggle, manual GPU disabling, and a hover tooltip summary of GPU states.
- A GUI frontend featuring more detailed GPU information and settings for the GUI and daemon.

## Install

Install the latest version using [my Flatpak repo](https://juandelpueblo.github.io/cardwire-tray/flatpak/me.edyan.cardwiretray.flatpakref). If you don't have Flatpak, build and compile using make.

Dependencies: `rust`, `cargo`, `cardwire` v0.10.0 or later 

```bash
make
sudo make install
```
