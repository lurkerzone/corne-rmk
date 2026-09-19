#![no_main]
#![no_std]

mod nice_view; // NEW

use rmk::macros::rmk_peripheral;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
     /// NEW: right-half nice!view.
    #[register_processor(event)]
    fn nice_view() -> crate::nice_view::NiceViewProcessor {
        crate::nice_view::processor()
    }
