#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
// features
#![feature(prelude_2024)]
#![feature(alloc_error_handler)]
#![feature(decl_macro)]
#![feature(auto_traits)]
#![feature(negative_impls)]
#![feature(panic_info_message)]
#![feature(exclusive_range_pattern)]
#![feature(const_mut_refs)]
#![feature(const_option)]

#[cfg(not(test))]
mod init;

extern crate alloc;

pub mod allocator;
pub mod console;
pub mod fs;
pub mod mutex;
pub mod shell;

use shell::shell;
use shim::io::Write;

use allocator::Allocator;
use fs::FileSystem;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();

pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();
use crate::console::kprintln;

use pi::atags::Atags;
fn kmain() -> ! {
    spin_sleep(Duration::from_millis(500)); // necessary delay after transmit before tty

    // let atags = Atags::get();
    // for a in atags {
    //     kprintln!("{:#?}", a);
    // }

    unsafe {
        ALLOCATOR.initialize();
        // FILESYSTEM.initialize();
    }

    use alloc::vec::Vec;

    // let mut v = Vec::new();
    // for i in 0..50 {
    //     v.push(i);
    //     kprintln!("{:?}", v);
    // }

    shell(">");
}

use pi::uart::MiniUart;

fn echo_uart() -> ! {
    let mut uart = MiniUart::new();
    loop {
        let byte = uart.read_byte();
        uart.write_byte(b'\n');
        uart.write(&[byte]).expect("error writing char");
        uart.flush().expect("error flushing");
    }
}

use core::time::Duration;
use pi::gpio::Gpio;
use pi::timer::spin_sleep;
fn loading_spinner() -> ! {
    let top_left = Gpio::new(5).into_output();
    let top_right = Gpio::new(6).into_output();
    let left = Gpio::new(16).into_output();
    let right = Gpio::new(13).into_output();
    let bottom_left = Gpio::new(19).into_output();
    let bottom_right = Gpio::new(26).into_output();

    let delay = Duration::from_millis(100);

    let mut pins = [top_left, top_right, right, bottom_right, bottom_left, left];
    let mut i: usize = 0;
    loop {
        pins[i].set();
        pins[((i as i32) - 1).rem_euclid(pins.len() as i32) as usize].clear();
        spin_sleep(delay);
        i = (i + 1) % pins.len();
    }
}

