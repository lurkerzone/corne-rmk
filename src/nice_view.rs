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

        Self { spi, cs, frame, vcom: false }
    }

    async fn write_frame(&mut self) {
        self.vcom = !self.vcom;
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
            let mut n = 0;
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
            let s: BatteryStatus = (*b).into();
            battery_level(s)
        })
    } else {
        None
    };
    put_text(d, "R", 4, 19, false, ON);
    battery_icon(d, 14, 18, 22, 11, right_lvl);
    put_text(d, pct_str(right_lvl, &mut pb), 42, 19, false, ON);

    // BLE profile circles (active one filled).
    let sel = ctx.ble_status.profile as usize;
    for i in 0..NUM_PROFILES {
        let x = 6 + (i as i32) * 20;
        let y = 36;
        let selected = i == sel;
        let circle = Circle::new(Point::new(x, y), 14);
        if selected {
            circle.into_styled(PrimitiveStyle::with_fill(ON)).draw(d).ok();
        } else {
            circle.into_styled(PrimitiveStyle::with_stroke(ON, 1)).draw(d).ok();
        }
        let digit = [b'1' + i as u8];
        let s = core::str::from_utf8(&digit).unwrap_or("?");
        put_text(d, s, x + 4, y + 2, false, if selected { OFF } else { ON });
    }
    let label = match ctx.ble_status.state {
        BleState::Connected => "CONNECTED",
        BleState::Advertising => "SEARCHING",
        BleState::Inactive => "USB / OFF",
    };
    put_text(d, label, 7, 54, false, ON);
}

/// Right half: own battery and link to the left half.
fn draw_peripheral_top<D: DrawTarget<Color = BinaryColor>>(ctx: &RenderContext, d: &mut D) {
    let mut pb = [0u8; 4];
    let own: BatteryStatus = ctx.battery.into();
    let lvl = battery_level(own);
    battery_icon(d, 4, 4, 34, 16, lvl);
    put_text(d, pct_str(lvl, &mut pb), 42, 8, false, ON);

    let link = if ctx.central_connected { "LINK: OK" } else { "LINK: --" };
    put_text(d, link, 4, 28, false, ON);
    put_text(d, "RIGHT", 4, 44, true, ON);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Central,
    Peripheral,
}

pub struct NiceViewRenderer {
    role: Role,
    samples: [u8; GRAPH_SAMPLES],
    last_sample: Instant,
}

impl NiceViewRenderer {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            samples: [0; GRAPH_SAMPLES],
            last_sample: Instant::from_ticks(0),
        }
    }

    /// Push a WPM sample every 2 seconds.
    fn sample(&mut self, wpm: u16) {
        let now = Instant::now();
        if now.duration_since(self.last_sample) >= Duration::from_secs(2) {
            self.samples.copy_within(1.., 0);
            self.samples[GRAPH_SAMPLES - 1] = wpm.min(255) as u8;
            self.last_sample = now;
        }
    }

    fn draw_bottom<D: DrawTarget<Color = BinaryColor>>(&self, ctx: &RenderContext, d: &mut D) {
        let mut nb = [0u8; 5];
        put_text(d, "WPM", 4, 70, false, ON);
        put_text(d, num_str(ctx.wpm, &mut nb), 28, 70, false, ON);
        if ctx.caps_lock {
            put_text(d, "C", 52, 70, false, ON);
        }
        if ctx.num_lock {
            put_text(d, "N", 60, 70, false, ON);
        }

        // WPM history graph.
        let max = self.samples.iter().copied().max().unwrap_or(0).max(30) as u32;
        for (i, s) in self.samples.iter().enumerate() {
            let h = (*s as u32 * GRAPH_H / max).max(1);
            fill(d, 4 + (i as i32) * 2, GRAPH_BOTTOM - h as i32, 2, h);
        }
        fill(d, 4, GRAPH_BOTTOM + 1, 60, 1);

        // Layer name.
        put_text(d, "LAYER", 4, 124, false, ON);
        let mut lb = [0u8; 5];
        let name = match LAYER_NAMES.get(ctx.layer as usize) {
            Some(n) => *n,
            None => num_str(ctx.layer as u16, &mut lb),
        };
        put_text(d, name, 4, 138, true, ON);
    }
}

impl DisplayRenderer<BinaryColor> for NiceViewRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(OFF).ok();
        if ctx.sleeping {
            return;
        }
        self.sample(ctx.wpm);
        match self.role {
            Role::Central => draw_central_top(ctx, display),
            Role::Peripheral => draw_peripheral_top(ctx, display),
        }
        self.draw_bottom(ctx, display);
    }
}

/// Short name so the entry files don't need generic types.
pub type NiceViewProcessor = DisplayProcessor<NiceView, NiceViewRenderer>;

/// Left half (central).
pub fn processor() -> NiceViewProcessor {
    DisplayProcessor::with_renderer(NiceView::new(), NiceViewRenderer::new(Role::Central))
        .with_render_interval(Duration::from_millis(1000))
}

/// Right half (peripheral).
pub fn processor_peripheral() -> NiceViewProcessor {
    DisplayProcessor::with_renderer(NiceView::new(), NiceViewRenderer::new(Role::Peripheral))
        .with_render_interval(Duration::from_millis(1000))
}
