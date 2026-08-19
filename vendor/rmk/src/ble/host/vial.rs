//! Vial over BLE GATT — disabled on this fork.
//!
//! Stock RMK adds a second HID-over-GATT service (`VialGattService`) next to
//! the real keyboard `HidService`. macOS binds to that extra HID service, so
//! Bluetooth shows Connected but keystrokes never arrive. USB Vial is unchanged
//! (`HostService` on the USB HID interface).

use trouble_host::prelude::*;

use super::HostWriteOutcome;
use crate::ble::ble_server::Server;
use crate::host::via::VialService;

/// Per-connection GATT write dispatcher. With Vial off BLE, every write is
/// unhandled so HID keyboard CCCD/report traffic is processed as usual.
pub(crate) struct HostGattHandler;

impl HostGattHandler {
    pub(crate) fn new(_server: &Server<'_>) -> Self {
        Self
    }

    pub(crate) async fn handle_write(&mut self, _handle: u16, _data: &[u8], _encrypted: bool) -> HostWriteOutcome {
        HostWriteOutcome::Unhandled
    }

    pub(crate) async fn run<'stack, 'server, P: PacketPool>(
        _server: &'server Server<'_>,
        _conn: &GattConnection<'stack, 'server, P>,
        _service: &VialService<'_>,
    ) {
        core::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vial_ble_is_disabled() {
        use crate::test_support::test_block_on as block_on;

        let server = Server::new_default("rmk").unwrap();
        let mut handler = HostGattHandler::new(&server);
        assert_eq!(
            block_on(handler.handle_write(u16::MAX, &[], true)),
            HostWriteOutcome::Unhandled
        );
    }
}
