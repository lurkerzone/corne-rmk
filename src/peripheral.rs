#![no_main]
#![no_std]

mod nice_view;

use rmk::macros::rmk_peripheral;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    /// Right-half nice!view.
    #[register_processor(event)]
    fn nice_view() -> crate::nice_view::NiceViewProcessor {
        crate::nice_view::processor()
    }
}
