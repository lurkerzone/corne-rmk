//! Sharp LS011B7DH03 (nice!view) driver + ZMK-style status screen for RMK.
//! Wiring: SCK = P0.20, MOSI = P0.17, CS = P0.06 (active HIGH).

use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::{self, InterruptExt, Priority};
use embassy_nrf::{Peripherals, bind_interrupts, peripherals, spim};
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X10};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::{Circle, PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use rmk::display::{DisplayDriver, DisplayProcessor, DisplayRenderer, RenderContext};
use rmk::types::battery::BatteryStatus;
use rmk::types::ble::BleState;

bind_interrupts!(struct SpiIrqs {
    SPI2 => spim::InterruptHandler<peripherals::SPI2>;
});

// ---- tweak these if the picture is wrong -----------------------------------
const PORTRAIT: bool = true;
const FLIP: bool = false;
const INVERT: bool = false;
// ---- screen content ----------------------------------------------------------
const NUM_PROFILES: usize = 3; // RMK default is 3 BLE profiles
const LAYER_NAMES: [&str; 4] = ["BASE", "LOWER", "RAISE", "FN"];
// -----------------------------------------------------------------------------

const NATIVE_W: usize = 160;
const NATIVE_H: usize = 68;
const LINE_BYTES: usize = NATIVE_W / 8;
const STRIDE: usize = LINE_BYTES + 2;
const FRAME_LEN: usize = 1 + NATIVE_H * STRIDE + 1;

const CMD_WRITE: u8 = 0x80;
const CMD_VCOM: u8 = 0x40;

pub struct NiceView {
    spi: spim::Spim<'static>,
    cs: Output<'static>,
    frame: [u8; FRAME_LEN],
    vcom: bool,
    last_vcom: Instant,
}

impl NiceView {
    pub fn new() -> Self {
        // SAFETY: nothing else in the firmware uses SPI2 or these pins.
        let p = unsafe { Peripherals::steal() };

        // The SoftDevice controller reserves NVIC priorities 0, 1 and 4.
        interrupt::SPI2.set_priority(Priority::P3);

        let mut cfg = spim::Config::default();
        cfg.frequency = spim::Frequency::M1;
        cfg.mode = spim::MODE_0;
        let spi = spim::Spim::new_txonly(p.SPI2, SpiIrqs, p.P0_20, p.P0_17, cfg);
        let cs = Output::new(p.P0_06, Level::Low, OutputDrive::Standard);

        let mut frame = [0xFFu8; FRAME_LEN];
        frame[0] = CMD_WRITE;
        for line in 0..NATIVE_H {
            let base = 1 + line * STRIDE;
            frame[base] = ((line + 1) as u8).reverse_bits();
            frame[base + STRIDE - 1] = 0;
        }
        frame[FRAME_LEN - 1] = 0;

        Self { spi, cs, frame, vcom: false, last_vcom: Instant::from_ticks(0) }
    }

    async fn write_frame(&mut self) {
        // Toggle VCOM about once a second, however often we redraw.
        let now = Instant::now();
        if now.duration_since(self.last_vcom) >= Duration::from_millis(1000) {
            self.vcom = !self.vcom;
            self.last_vcom = now;
        }
        self.frame[0] = CMD_WRITE | if self.vcom { CMD_VCOM } else { 0 };

        self.cs.set_high();
        Timer::after_micros(6).await;
        for chunk in self.frame.chunks(128) {
            if self.spi.write(chunk).await.is_err() {
                break;
            }
        }
        Timer::after_micros(4).await;
        self.cs.set_low();
    }
}

impl OriginDimensions for NiceView {
    fn size(&self) -> Size {
        if PORTRAIT {
            Size::new(NATIVE_H as u32, NATIVE_W as u32)
        } else {
            Size::new(NATIVE_W as u32, NATIVE_H as u32)
        }
    }
}

impl DrawTarget for NiceView {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<BinaryColor>>,
    {
        let sz = self.size();
        for Pixel(pt, color) in pixels {
            if pt.x < 0 || pt.y < 0 || pt.x >= sz.width as i32 || pt.y >= sz.height as i32 {
                continue;
            }
            let (x, y) = (pt.x as usize, pt.y as usize);
            let (nx, ny) = match (PORTRAIT, FLIP) {
                (false, false) => (x, y),
                (false, true) => (NATIVE_W - 1 - x, NATIVE_H - 1 - y),
                (true, false) => (y, NATIVE_H - 1 - x),
                (true, true) => (NATIVE_W - 1 - y, x),
            };
            let black = (color == BinaryColor::On) != INVERT;
            let idx = 1 + ny * STRIDE + 1 + nx / 8;
            let mask = 0x80u8 >> (nx % 8);
            if black {
                self.frame[idx] &= !mask;
            } else {
                self.frame[idx] |= mask;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: BinaryColor) -> Result<(), Self::Error> {
        let fill = if (color == BinaryColor::On) != INVERT { 0x00 } else { 0xFF };
        for line in 0..NATIVE_H {
            let b = 1 + line * STRIDE + 1;
            self.frame[b..b + LINE_BYTES].fill(fill);
        }
        Ok(())
    }
}

impl DisplayDriver for NiceView {
    async fn init(&mut self) {
        self.write_frame().await;
    }
    async fn flush(&mut self) {
        self.write_frame().await;
    }
}

// ============================ status screen ==================================

const ON: BinaryColor = BinaryColor::On;
const OFF: BinaryColor = BinaryColor::Off;

const GRAPH_SAMPLES: usize = 30; // 30 samples x 2 px = 60 px wide
const GRAPH_H: u32 = 30;
const GRAPH_BOTTOM: i32 = 116;

fn put_text<D: DrawTarget<Color = BinaryColor>>(
    d: &mut D,
    s: &str,
    x: i32,
    y: i32,
    big: bool,
    color: BinaryColor,
) {
    let pos = Point::new(x, y);
    if big {
        let style = MonoTextStyle::new(&FONT_10X20, color);
        Text::with_baseline(s, pos, style, Baseline::Top).draw(d).ok();
    } else {
        let style = MonoTextStyle::new(&FONT_6X10, color);
        Text::with_baseline(s, pos, style, Baseline::Top).draw(d).ok();
    }
}

fn fill<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, y: i32, w: u32, h: u32) {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
        .into_styled(PrimitiveStyle::with_fill(ON))
        .draw(d)
        .ok();
}

fn outline<D: DrawTarget<Color = BinaryColor>>(d: &mut D, x: i32, y: i32, w: u32, h: u32) {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
        .into_styled(PrimitiveStyle::with_stroke(ON, 1))
        .draw(d)
        .ok();
}

fn battery_level(status: BatteryStatus) -> Option<u8> {
    match status {
        BatteryStatus::Available { level: Some(l), .. } => Some(l),
        _ => None,
    }
}

fn battery_icon<D: DrawTarget<Color = BinaryColor>>(
    d: &mut D,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    level: Option<u8>,
) {
    outline(d, x, y, w, h);
    fill(d, x + w as i32, y + (h / 4) as i32, 2, h / 2);
    match level {
        Some(l) => {
            let inner = (w - 4) * (l.min(100) as u32) / 100;
            if inner > 0 {
                fill(d, x + 2, y + 2, inner, h - 4);
            }
        }
        None => put_text(d, "?", x + (w as i32) / 2 - 3, y + (h as i32) / 2 - 5, false, ON),
    }
}

fn pct_str(level: Option<u8>, buf: &mut [u8; 4]) -> &str {
    match level {
        None => "--",
        Some(l) => {
            let l = l.min(100);
            let n;
            if l >= 100 {
                buf[0] = b'1';
                buf[1] = b'0';
                buf[2] = b'0';
                n = 3;
            } else if l >= 10 {
                buf[0] = b'0' + l / 10;
                buf[1] = b'0' + l % 10;
                n = 2;
            } else {
                buf[0] = b'0' + l;
                n = 1;
            }
            buf[n] = b'%';
            core::str::from_utf8(&buf[..n + 1]).unwrap_or("?")
        }
    }
}

fn num_str(mut n: u16, buf: &mut [u8; 5]) -> &str {
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    core::str::from_utf8(&buf[i..]).unwrap_or("?")
}

/// Left half: both batteries, BLE profiles and connection state.
fn draw_central_top<D: DrawTarget<Color = BinaryColor>>(ctx: &RenderContext, d: &mut D) {
    let mut pb = [0u8; 4];

    let own: BatteryStatus = ctx.battery.into();
    let own_lvl = battery_level(own);
    put_text(d, "L", 4, 3, false, ON);
    battery_icon(d, 14, 2, 22, 11, own_lvl);
    put_text(d, pct_str(own_lvl, &mut pb), 42, 3, false, ON);

    let right_connected = ctx.peripherals_connected.first().copied().unwrap_or(false);
    let right_lvl = if right_connected {
        ctx.peripheral_batteries.first().and_then(|b| {
            let s: BatteryStatus =
