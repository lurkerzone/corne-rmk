//! Sharp LS011B7DH03 (nice!view) driver for RMK's DisplayProcessor.
//! Wiring (Corne OLED header + one bodge wire):
//!   SCK  = P0.20 (SCL pad)   MOSI = P0.17 (SDA pad)   CS = P0.06 (pro-micro D1, active HIGH)

use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::{self, InterruptExt, Priority};
use embassy_nrf::{Peripherals, bind_interrupts, peripherals, spim};
use embassy_time::{Duration, Timer};
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X10};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use rmk::display::{DisplayDriver, DisplayProcessor, DisplayRenderer, RenderContext};

bind_interrupts!(struct SpiIrqs {
    SPI2 => spim::InterruptHandler<peripherals::SPI2>;
});

// ---- tweak these if the picture is wrong -----------------------------------
const PORTRAIT: bool = true; // true: 68 wide x 160 tall (vertical, like ZMK)
const FLIP: bool = true; // rotate 180 degrees if the image is upside down
const INVERT: bool = false; // flip if you get white-on-black instead of black-on-white
// -----------------------------------------------------------------------------

const NATIVE_W: usize = 160;
const NATIVE_H: usize = 68;
const LINE_BYTES: usize = NATIVE_W / 8; // 20
const STRIDE: usize = LINE_BYTES + 2; // line address + data + trailer
const FRAME_LEN: usize = 1 + NATIVE_H * STRIDE + 1;

// The panel wants LSB-first; SPIM is MSB-first, so bytes are stored bit-reversed.
const CMD_WRITE: u8 = 0x80; // M0 (sent first)
const CMD_VCOM: u8 = 0x40; // M1

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
        cfg.frequency = spim::Frequency::M1; // panel max is 1 MHz
        cfg.mode = spim::MODE_0;
        let spi = spim::Spim::new_txonly(p.SPI2, SpiIrqs, p.P0_20, p.P0_17, cfg);
        let cs = Output::new(p.P0_06, Level::Low, OutputDrive::Standard);

        let mut frame = [0xFFu8; FRAME_LEN]; // all white
        frame[0] = CMD_WRITE;
        for line in 0..NATIVE_H {
            let base = 1 + line * STRIDE;
            frame[base] = ((line + 1) as u8).reverse_bits(); // 1-based line address
            frame[base + STRIDE - 1] = 0; // trailer
        }
        frame[FRAME_LEN - 1] = 0;

        Self { spi, cs, frame, vcom: false }
    }

    async fn write_frame(&mut self) {
        // VCOM must toggle regularly or the panel degrades.
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

/// Turn a number into text without needing the heapless crate.
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

/// Simple high-contrast screen sized for 68x160: border, layer, WPM, Caps/Num.
#[derive(Default)]
pub struct NiceViewRenderer;

impl DisplayRenderer<BinaryColor> for NiceViewRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();
        if ctx.sleeping {
            return;
        }

        let small = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let big = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let mut buf = [0u8; 5];

        // Border: shows whether the whole canvas is visible and oriented correctly.
        let size = display.bounding_box().size;
        Rectangle::new(Point::zero(), size)
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .ok();

        Text::new("LAYER", Point::new(6, 18), small).draw(display).ok();
        Text::new(num_str(ctx.layer as u16, &mut buf), Point::new(6, 42), big)
            .draw(display)
            .ok();

        Text::new("WPM", Point::new(6, 72), small).draw(display).ok();
        Text::new(num_str(ctx.wpm, &mut buf), Point::new(6, 96), big)
            .draw(display)
            .ok();

        if ctx.caps_lock {
            Text::new("CAPS", Point::new(6, 124), small).draw(display).ok();
        }
        if ctx.num_lock {
            Text::new("NUM", Point::new(6, 138), small).draw(display).ok();
        }
    }
}

/// Short name so the entry files don't need generic types.
pub type NiceViewProcessor = DisplayProcessor<NiceView, NiceViewRenderer>;

pub fn processor() -> NiceViewProcessor {
    DisplayProcessor::with_renderer(NiceView::new(), NiceViewRenderer)
        .with_render_interval(Duration::from_millis(1000))
}
