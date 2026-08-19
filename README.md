# Corne RMK

Wireless split Corne (6-column) for **nice!nano v2** (nRF52840, Adafruit UF2 bootloader). Pinout matches the ZMK Corne shield + nice!nano `pro_micro` map.

Firmware is **RMK 0.9** (`rmk-rs/rmk`, vendored under `vendor/` with a small patch). Tags: `rmk-v0.8` (previous), `rmk-v0.9` (current).

| Half | Flash | Role |
|------|--------|------|
| Left | `corne-rmk-central.uf2` | USB + BLE to the Mac; BLE split to the right |
| Right | `corne-rmk-peripheral.uf2` | BLE split to the left only |

Swap which half gets which UF2 if your USB side is the right.

## What works

- USB and Bluetooth typing (unplug the cable; BLE is used automatically)
- Battery % for **both** halves over BLE (host Battery Service)
- **Vial over USB only** (macOS mis-binds a second HID-over-GATT service). Unlock: **LGui + Lower**. Plug into the left half.
- Panic/HardFault reboots into the UF2 bootloader (`NICENANO`)

## Build

Rust 1.85+ (edition 2024). From the repo root:

```bash
rustup target add thumbv7em-none-eabihf
rustup component add llvm-tools
cargo install cargo-make flip-link cargo-binutils cargo-hex-to-uf2
cargo make uf2
```

That writes `corne-rmk-central.uf2` and `corne-rmk-peripheral.uf2` (gitignored). CI builds the same artifacts (Actions → `corne-rmk-uf2`).

## Flash

Double-tap RST, copy the matching UF2 onto `NICENANO`. Finder “device disappeared” / `cp: Device not configured` is normal: the bootloader jumps to the app.

A normal reflash keeps the chip’s BLE identity (from nRF `FICR`). Forget and re-pair only after a GATT change or a `clear_storage` wipe.

## Config

- [`keyboard.toml`](keyboard.toml) — pins, layers, Vial unlock keys, battery (`vddh` on both halves)
- [`vial.json`](vial.json) — Vial layout; rebuild after editing
- BLE addresses in toml are unused on nRF52; each board gets a unique address from the chip. Two Cornes in the same room do not need different `ble_addr` values.

Storage wipe (bonds + Vial saves): set `clear_storage = true` under `[storage]`, flash both halves, boot once, then comment it out and flash again.

## Upstream

- [rmk-rs/rmk](https://github.com/rmk-rs/rmk) — local copy and USB-only Vial patch: [`vendor/README.md`](vendor/README.md)
- [RMK docs](https://rmk.rs/docs/user_guide/guide_overview)
