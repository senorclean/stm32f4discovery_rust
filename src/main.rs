// #![deny(unsafe_code)]
#![no_main]
#![no_std]

// halt on panic
#[allow(unused_extern_crates)]
// extern crate panic_halt;

// use cortex_m;
// use cortex_m_rt::entry;
use stm32f4xx_hal::{
  prelude::*,
  stm32,
  gpio::gpiod,
  gpio::Output,
  gpio::PushPull,
  delay::Delay,
};

use rtic::cyccnt::{Instant, U32Ext};

use cortex_m_semihosting::hprintln;
use panic_semihosting as _;

#[rtic::app(
  device = stm32f4xx_hal::stm32,
  peripherals = true,
  monotonic = rtic::cyccnt::CYCCNT
)]
const APP: () = {
  struct Resources {
    led: gpiod::PD12<Output<PushPull>>,
  }

  #[init(schedule = [blink])]
  fn init(cx: init::Context) -> init::LateResources {
    // cortex-m peripherals
//     let core: cortex_m::Peripherals = cx.core;
// 
//     core.DCB.enable_trace();
//     core.DWT.enable_cycle_counter();
// 
//     let now = cx.start;
// 
//     let addr = 0xE000EDF0usize;
//     let r = addr as *const u32;
//     let debug_status = unsafe{*r & 0x00000001};
// 
//     if debug_status == 1 {
//       hprintln!("init @ {:?}", now).unwrap();
//     }

    // device specific peripherals
    // let _device: stm32::Peripherals = cx.device;
    let device: stm32::Peripherals = cx.device;

    let gpiod = device.GPIOD.split();
    let led = gpiod.pd12.into_push_pull_output();

    let rcc = device.RCC.constrain();
    rcc.cfgr.sysclk(168.mhz()).freeze();

    cx.schedule.blink(cx.start + 1_000.cycles()).unwrap();

    init::LateResources {
      led,
    }
  }

  #[task(schedule = [blink], resources = [led])]
  fn blink(cx: blink::Context) {
    // let now = Instant::now();

    let high = cx.resources.led.is_set_high().unwrap();
    if high {
      cx.resources.led.set_low().unwrap();
    } else {
      cx.resources.led.set_high().unwrap();
    }

    cx.schedule.blink(cx.scheduled + 168_000_000.cycles()).unwrap();
  }

//   #[idle(resources = [led, delay])]
//   fn idle(cx: idle::Context) -> ! {
// 
//     loop {
//       cx.resources.led.set_high().unwrap();
//       cx.resources.delay.delay_ms(1000_u32);
//       cx.resources.led.set_low().unwrap();
//       cx.resources.delay.delay_ms(1000_u32);
//     }
//   }

  extern "C" {
    fn SDIO();
  }
};

