# Corne RMK (nRF52840)

RMK firmware for a **wireless split Corne** (foostan **6-column** layout) on **nice!nano-class** boards (nRF52840, Adafruit UF2 bootloader). The matrix pinout matches the official ZMK **Corne** shield plus the **nice!nano** `pro_micro` pin map.

## Roles

| Half   | Flash this binary | Notes                                      |
|--------|-------------------|--------------------------------------------|
| **Left**  | `central`   | USB to the host; BLE to the computer       |
| **Right** | `peripheral`| Talks to the left half over BLE split only |

If you prefer the right half as the USB side, swap which physical half gets which firmware (and adjust `keyboard.toml` offsets if you also swap how you think about “left” in the keymap).

## Build firmware locally

You need a **recent stable Rust** (edition **2024** requires roughly **1.85+**). Install via [rustup](https://rustup.rs) if you do not have `cargo` yet.

From the repository root:

```bash
cd /path/to/corne_rmk

# Cortex-M4F target for nRF52840
rustup target add thumbv7em-none-eabihf

# Used by `cargo objcopy` in the UF2 pipeline (also installed automatically by some cargo-make tasks)
rustup component add llvm-tools

# One-time: tools for UF2 generation (cargo-make can install these too, but pre-installing is clearer)
cargo install cargo-make flip-link cargo-binutils cargo-hex-to-uf2

# Produce both UF2s in the repo root
cargo make uf2 --release
```

Artifacts:

- `corne-rmk-central.uf2` → **left** half (central / USB to host).
- `corne-rmk-peripheral.uf2` → **right** half (peripheral).

Intermediate Intel HEX files (`corne-rmk-*.hex`) are written during the UF2 step; they are gitignored. Remove them with **`cargo make clean-hex`** when you only care about the `.uf2` files.

### Build ELF only (no UF2)

```bash
cargo build --release --bin central
cargo build --release --bin peripheral
```

Binaries land under `target/thumbv7em-none-eabihf/release/`.

Optional: [probe-rs](https://probe.rs/) if you flash over SWD instead of UF2.

## UF2 for nice!nano (drag-and-drop)

1. Ensure [`memory.x`](memory.x) uses **FLASH at `0x00001000`** when the **Adafruit nRF52 bootloader** is present (default in this repo).
2. Run **`cargo make uf2 --release`** (see [Build firmware locally](#build-firmware-locally)).
3. Double-tap reset to enter the bootloader, copy the matching UF2, then **unplug USB** when testing BLE (RMK can prefer USB when a cable is connected).

## GitHub Actions

Workflow [`.github/workflows/firmware.yml`](.github/workflows/firmware.yml) runs on pushes and pull requests to **`main`** / **`master`**, and on **manual dispatch**. It builds the same UF2s and uploads them as the **`corne-rmk-uf2`** artifact (download from the run’s **Actions** → **Artifacts**).

## Flash with a debug probe

With `memory.x` matching your bootloader (or full-chip start at `0x0` if no bootloader):

```bash
cargo run --release --bin central
cargo run --release --bin peripheral
```

The default runner in [`.cargo/config.toml`](.cargo/config.toml) is `probe-rs run --chip nRF52840_xxAA`.

## Configuration

- **[`keyboard.toml`](keyboard.toml)** — matrix pins, split geometry (`4×6` + `4×6`, `col_offset = 6`), layers, and keymap. The stock keymap has **four layers** (0–3 in Vial). Layer **3** starts empty; **`MO(3)`** is on **Raise** on the key between **Enter** and **RAlt** (hold **Raise**, then that thumb key). Change or remove that in Vial / TOML as you like.
- **[`vial.json`](vial.json)** — Vial **visual layout** (staggered Corne / crkbd) and custom keycodes (e.g. Bluetooth helpers). The key positions come from [vial-qmk’s crkbd `vial.json`](https://github.com/vial-kb/vial-qmk/blob/master/keyboards/crkbd/keymaps/vial/vial.json), with matrix labels remapped from QMK’s **8×6 split** indices to this firmware’s **4×12** grid (right half: row `r−4`, column `11−c`). After editing `vial.json`, **rebuild and reflash**—it is compiled into the firmware via `build.rs`.

### Bluetooth channels / profiles in Vial

There is no separate “Bluetooth” tab in Vial. This repo already lists the actions in [`vial.json`](vial.json) under **`customKeycodes`** (`BT0` / `BT1` / `BT2`, **Next BT**, **Prev BT**, **Clear BT**, **Switch Output**, **Clear Peer**). They map to RMK’s internal **User** keycodes (`0x7E00`–`0x7E07`).

**How to assign them:** click a key in Vial, open the key picker, then:

- Use the **search / filter** (if your Vial build has one) and try **`BT`**, **`Bluetooth`**, **`Next`**, or **`User`**.
- Scroll the categories—custom keys often appear as **User 0**, **User 1**, … or under the **short names** from `vial.json` (e.g. **BT0**), depending on Vial version and platform (desktop vs web).

Use the **Vial** app (not plain Via). **Unlock** Vial (GUI + Lower) before saving assignments.

**Behavior (RMK):** **BT0–BT2** switch host slot on **key release**. **Clear Peer** on a split board is triggered by **holding** that key for **~5 seconds** (see RMK source). Default build uses **3** BLE slots; you can change that with **`ble_profiles_num`** under **`[rmk]`** in `keyboard.toml` (then add matching **`BTn`** entries in `vial.json` and rebuild). See [RMK `rmk` config](https://rmk.rs/docs/configuration/rmk_config) (`ble_profiles_num`) and [wireless / BLE](https://rmk.rs/docs/configuration/wireless).

### Mouse keys

**Yes.** RMK supports **mouse movement, buttons, and wheel** over USB and BLE (composite HID). In [`keyboard.toml`](keyboard.toml) you can use names like **`MouseUp`**, **`MouseDown`**, **`MouseLeft`**, **`MouseRight`**, **`MouseBtn1`**–**`MouseBtn8`**, **`MouseWheelUp`**, **`MouseWheelDown`**, etc. (see [RMK keycodes](https://rmk.rs/docs/configuration/keymap_configuration/keycodes)). If your Vial build exposes a **Mouse** section in the picker, those map to the same HID codes; otherwise set them in the TOML default map or use any key picker entry that matches the underlying code.

### Vial changes not sticking?

**1. Unlock Vial after each connect / reflash**  
RMK ships with **`vial_lock` on**. The Vial app will not reliably **save** until the keyboard is unlocked: in Vial start **Unlock**, then hold the **physical** keys defined in **`[host] unlock_keys`** in [`keyboard.toml`](keyboard.toml) (default: matrix **(3,3)** and **(3,4)** — left-thumb **LGui** and **Lower** on the stock map). If you remapped those matrix positions, either change `unlock_keys` to two keys you can still press together, or disable `vial_lock` in [`Cargo.toml`](Cargo.toml) (see RMK docs; tradeoff: no unlock gesture).

**2. Reflashing usually wipes saved layout**  
A UF2 update rewrites application flash. RMK stores Vial/keymap data in flash; after a flash that data is often **gone or invalid**. Treat **saved Vial layout as reset** after reflashing: connect USB, **unlock Vial**, then reconfigure (defaults still come from [`keyboard.toml`](keyboard.toml)).

**3. Avoid loading a `.vil` from another keyboard**  
A file from **another** Corne (QMK, ZMK, different `vial.json`, etc.) targets a **different** keyboard definition. Vial may error; continuing can write **incompatible** data and break persistence. Configure **this** firmware in Vial from scratch, or only import a layout exported from **this same** RMK build.

**4. Reset corrupted storage (one-shot)**  
If a bad import left storage inconsistent, set **`clear_storage = true`** under **`[storage]`** in [`keyboard.toml`](keyboard.toml), build, flash, boot **once**, then **remove** that line and flash again. You will need to **re-pair Bluetooth** afterward.

### Bluetooth won’t connect after flashing?

That is **usually unrelated to Vial / `vial_lock`**, which does not touch the BLE stack. Typical fixes:

1. **Forget / remove** the keyboard from the host’s Bluetooth devices, then pair again (bonding data often mismatches after firmware changes).
2. Flash **both** `central` and `peripheral` UF2s to the correct halves; both need power (right half is BLE-only to the left).
3. For BLE testing, **unplug USB** from the central half (RMK may prefer USB when connected).
4. If you use custom Bluetooth keys, try **clear bond** / channel reset on the keyboard side, then re-pair.

### Pins and battery

Row and column pins are derived from ZMK’s Corne + nice!nano mapping. If your PCB or controller differs, update `[split.central.matrix]` and `[split.peripheral.matrix]`.

### Battery reporting (BLE)

**You already have this wired in** [`keyboard.toml`](keyboard.toml): both **`[split.central]`** and **`[[split.peripheral]]`** set **`battery_adc_pin = "P0_04"`** (typical **nice!nano** LiPo sense) plus **`adc_divider_measured = 2000`** and **`adc_divider_total = 2806`** for the usual **806 kΩ / 2 MΩ** divider. RMK uses the nRF52 **SAADC** and, over Bluetooth, exposes the standard **Battery Service (BAS)** so the **host can show a percentage** when it supports BLE battery levels (behavior varies by OS and whether you are connected over **BLE vs USB**).

**If the OS never shows a level:** confirm you are paired over **Bluetooth** (some systems only surface battery for the active BLE connection), check that **`P0_04`** matches your board’s schematic, and **retune** the two `adc_*` values if the percentage is consistently wrong. You can use **`battery_adc_pin = "vddh"`** to read chip supply instead of the divider pin (see [RMK wireless / `[ble]`](https://rmk.rs/docs/configuration/wireless)); that is simpler but **not** the same as true pack voltage on a divided sense line.

**Split note:** RMK’s docs mention a limitation where **central and peripheral currently share the same ADC-related config** in some setups—if you hit odd readings on one half, check [RMK wireless docs](https://rmk.rs/docs/configuration/wireless) and upstream issues for updates.

### Multiple keyboards

Change **`ble_addr`** under `[split.central]` and `[[split.peripheral]]` to a **new unique pair** for each Corne so halves do not cross-pair.

### Bootloader vs no bootloader

See comments in [`memory.x`](memory.x): use **0x1000** with Adafruit-style bootloader, or **0x0** for full flash if you removed it.

## Layout source

ZMK reference keymap: `app/boards/shields/corne/corne.keymap` in the [ZMK tree](https://github.com/zmkfirmware/zmk). This project mirrors the default / lower / raise layers in RMK action syntax.

## Upstream

- [RMK](https://github.com/HaoboGu/rmk) — this repo pins `rmk` to a specific git revision in [`Cargo.toml`](Cargo.toml) so it stays aligned with the `nrf52840_ble_split` example dependency stack. Bump the `rev` when you intentionally upgrade RMK.
- [RMK docs](https://rmk.rs/docs/user_guide/guide_overview)