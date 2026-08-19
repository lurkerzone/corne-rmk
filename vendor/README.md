# Vendored RMK (USB-only Vial)

Source: [rmk-rs/rmk](https://github.com/rmk-rs/rmk) commit `f8da2742971d048e0080e57d69d98f00a859e4e9`.

## Why vendor

Stock RMK with `vial` + BLE registers **two** HID-over-GATT services:

1. `HidService` — keyboard / mouse / media reports
2. `VialGattService` — Vial protocol, same HID UUID

macOS often attaches to the Vial HID service. Bluetooth then shows Connected, USB typing still works, and wireless keys do nothing.

This fork keeps USB Vial (`vial` + `host_lock`) and **does not advertise** `VialGattService`. Battery Service (BAS) is unchanged.

## Patch

- `rmk/src/ble/ble_server.rs` — `Server` no longer includes `vial_service`
- `rmk/src/ble/host/vial.rs` — BLE Vial handler is a no-op
