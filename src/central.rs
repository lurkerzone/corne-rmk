#![no_main]
#![no_std]

mod nice_view; // NEW

use embassy_nrf::usb::vbus_detect::SoftwareVbusDetect;
use embassy_time::{Duration, Timer};
use rmk::macros::rmk_central;
use rmk::state::set_usb_state;
use rmk::types::connection::UsbState;

/// Poll nRF POWER.USBREGSTATUS and push changes into SoftwareVbusDetect.
///
/// RMK cannot use HardwareVbusDetect together with nrf-sdc (POWER/CLOCK is
/// owned by MPSL). When the cable is pulled, the USB stack often sits in
/// `Suspended` — and RMK still treats Suspended as a live USB HID path, so
/// keystrokes never go to the BLE connection. Force USB `Disabled` as soon as
/// VBUS is gone so BLE becomes the active transport.
#[embassy_executor::task]
async fn vbus_poll(vbus: &'static SoftwareVbusDetect) {
    let status = embassy_nrf::pac::POWER.usbregstatus().read();
    let mut last_detected = status.vbusdetect();
    let mut last_ready = status.outputrdy();
    loop {
        Timer::after(Duration::from_millis(50)).await;
        let status = embassy_nrf::pac::POWER.usbregstatus().read();
        let detected = status.vbusdetect();
        let ready = status.outputrdy();
        if detected != last_detected {
            vbus.detected(detected);
            last_detected = detected;
            last_ready = false;
        }
        if detected && ready && !last_ready {
            vbus.ready();
            last_ready = true;
        }
        if !detected {
            set_usb_state(UsbState::Disabled);
        }
    }
}

#[rmk_central]
mod keyboard_central {
    /// NEW: left-half nice!view.
    #[register_processor(event)]
    fn nice_view() -> ::rmk::display::DisplayProcessor
        crate::nice_view::NiceView,
        ::rmk::display::OledRenderer,
    > {
        crate::nice_view::processor()
    }

    /// Software VBUS detect + a poller, instead of HardwareVbusDetect.
    #[Override(usb)]
    fn usb() {
        {
            static VBUS: ::static_cell::StaticCell<::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect> =
                ::static_cell::StaticCell::new();
            let status = ::embassy_nrf::pac::POWER.usbregstatus().read();
            let vbus = VBUS.init(::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect::new(
                status.vbusdetect(),
                status.outputrdy(),
            ));
            spawner.spawn(crate::vbus_poll(vbus).unwrap());
            ::embassy_nrf::usb::Driver::new(p.USBD, Irqs, &*vbus)
        }
    }
}#![no_main]
#![no_std]

use embassy_nrf::usb::vbus_detect::SoftwareVbusDetect;
use embassy_time::{Duration, Timer};
use rmk::macros::rmk_central;
use rmk::state::set_usb_state;
use rmk::types::connection::UsbState;

/// Poll nRF POWER.USBREGSTATUS and push changes into SoftwareVbusDetect.
///
/// RMK cannot use HardwareVbusDetect together with nrf-sdc (POWER/CLOCK is
/// owned by MPSL). When the cable is pulled, the USB stack often sits in
/// `Suspended` — and RMK still treats Suspended as a live USB HID path, so
/// keystrokes never go to the BLE connection. Force USB `Disabled` as soon as
/// VBUS is gone so BLE becomes the active transport.
#[embassy_executor::task]
async fn vbus_poll(vbus: &'static SoftwareVbusDetect) {
    let status = embassy_nrf::pac::POWER.usbregstatus().read();
    let mut last_detected = status.vbusdetect();
    let mut last_ready = status.outputrdy();
    loop {
        Timer::after(Duration::from_millis(50)).await;
        let status = embassy_nrf::pac::POWER.usbregstatus().read();
        let detected = status.vbusdetect();
        let ready = status.outputrdy();

        if detected != last_detected {
            vbus.detected(detected);
            last_detected = detected;
            last_ready = false;
        }
        if detected && ready && !last_ready {
            vbus.ready();
            last_ready = true;
        }
        if !detected {
            set_usb_state(UsbState::Disabled);
        }
    }
}

#[rmk_central]
mod keyboard_central {
        /// NEW: left-half nice!view.
    #[register_processor(event)]
    fn nice_view() -> ::rmk::display::DisplayProcessor
        crate::nice_view::NiceView,
        ::rmk::display::OledRenderer,
    > {
        crate::nice_view::processor()
    }
    /// Software VBUS detect + a poller, instead of HardwareVbusDetect.
    #[Override(usb)]
    fn usb() {
        {
            static VBUS: ::static_cell::StaticCell<::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect> =
                ::static_cell::StaticCell::new();
            let status = ::embassy_nrf::pac::POWER.usbregstatus().read();
            let vbus = VBUS.init(::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect::new(
                status.vbusdetect(),
                status.outputrdy(),
            ));
            spawner.spawn(crate::vbus_poll(vbus).unwrap());
            ::embassy_nrf::usb::Driver::new(p.USBD, Irqs, &*vbus)
        }
    }
}
