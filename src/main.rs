#![deny(unsafe_code)]
#![no_main]
#![no_std]

// halt on panic
#[allow(unused_extern_crates)]
extern crate panic_halt;

use cortex_m;
use cortex_m_rt::entry;
use stm32f4xx_hal as hal;

use crate::hal::{prelude::*, stm32};

#[entry]
fn main() -> ! {
  if let (Some(dp), Some(cp)) = (
    stm32::Peripherals::take(),
    cortex_m::peripheral::Peripherals::take(),
  ) {
    // setup the led
    let gpiod = dp.GPIOD.split();
    let mut led = gpiod.pd12.into_push_pull_output();

    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(168.mhz()).freeze();

    let mut delay = hal::delay::Delay::new(cp.SYST, clocks);

    loop {
      led.set_high().unwrap();
      delay.delay_ms(1000_u32);
      led.set_low().unwrap();
      delay.delay_ms(1000_u32);
    }
  }

  loop {}
}
