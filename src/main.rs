// #![deny(unsafe_code)]
#![no_main]
#![no_std]
#![feature(destructuring_assignment)]

#[allow(unused_extern_crates)]

mod heartbeat;
mod util;
mod constants;
mod button;

#[rtic::app(
  device = stm32f4xx_hal::stm32,
  peripherals = true,
  monotonic = rtic::cyccnt::CYCCNT,
  dispatchers = [SDIO, CRYP]
)]
mod app {
  use super::*;
  use cortex_m::asm;
  use stm32f4xx_hal::{
    prelude::*,
    stm32,
    pwm,
    gpio::gpioa,
    // gpio::gpiod,
    // gpio::Output,
    gpio::Input,
    gpio::Floating,
    // gpio::PushPull,
    gpio::Edge,
    gpio::ExtiPin,
    pwm::C2,
    stm32::TIM4,
    // delay::Delay,
  };
  use rtic_core::prelude::*;
  use rtic::cyccnt::{Instant, U32Ext};

  use cortex_m_semihosting::hprintln;
  use panic_semihosting as _;

  #[resources]
  struct Resources<T, U> {
    // led: gpiod::PD12<Output<PushPull>>,
    heartbeat: heartbeat::Data<TIM4, C2>,
    button: button::Data<gpioa::PA0<Input<Floating>>>,
    exti: stm32::EXTI,
    debugger: bool,
  }

  #[init()]
  fn init(cx: init::Context) -> init::LateResources {

    let debugger = util::is_debugger_connected();
    if debugger {
      hprintln!("init").unwrap();
    };

    // device specific peripherals
    let device: stm32::Peripherals = cx.device;
    let mut syscfg = device.SYSCFG;
    let mut exti = device.EXTI;

    let rcc = device.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(168.mhz()).freeze();

    let gpioa = device.GPIOA.split();
    let gpiod = device.GPIOD.split();
    // let led = gpiod.pd12.into_push_pull_output();

    // configure button as interrupt source
    let mut button = gpioa.pa0.into_floating_input();
    button.make_interrupt_source(&mut syscfg);
    button.trigger_on_edge(&mut exti, Edge::RISING);
    button.enable_interrupt(&mut exti);

    // configure PWM module
    let pwm_channel = gpiod.pd13.into_alternate_af2();
    let mut heartbeat_led= pwm::tim4(device.TIM4, pwm_channel, clocks, 20u32.khz());
    heartbeat_led.set_duty(0);

    // start tasks
    heartbeat_mb_app::spawn(heartbeat::Message::TurnOn).unwrap();

    // initialize resource data
    let button= button::Data::new(button);
    let heartbeat = heartbeat::Data::new(heartbeat_led);

    init::LateResources {
      // led,
      heartbeat,
      button,
      exti,
      debugger
    }
  }

  #[task(priority = 3, binds = EXTI0, resources = [button, exti])]
  fn exti0(cx: exti0::Context) {
    match button_app::schedule(Instant::now() + util::convert_us_to_cycles(10_000).cycles()) {
      Ok(_) => (),
      // function is likely already scheduled
      Err(_) => ()
    }

    let exti = cx.resources.exti;
    let button = cx.resources.button;
    
    (button, exti).lock(|button, exti| {
        button.button.clear_interrupt_pending_bit();
        button.button.disable_interrupt(exti);
    });
  }

  #[task(priority = 2, resources = [button, debugger])]
  fn button_mb_app(cx: button_mb_app::Context, msg: button::Message) {
    button::button_mb(cx, msg);
  }

  #[task(resources = [button, exti, debugger])]
  fn button_app(cx: button_app::Context) {
    button::button(cx);
  }

  #[task(priority = 2, resources = [heartbeat, debugger])]
  fn heartbeat_mb_app(cx: heartbeat_mb_app::Context, msg: heartbeat::Message) {
    heartbeat::heartbeat_mb(cx, msg);
  }

  #[task(resources = [heartbeat])]
  fn heartbeat_app(cx: heartbeat_app::Context, increment: bool) {
    heartbeat::heartbeat(cx, increment);
  }

  #[idle()]
  fn idle(_cx: idle::Context) -> ! {
    loop {
      // sleep while waiting for next event
      asm::wfi();
    }
  }

  // #[task(schedule = [blink], resources = [led])]
  // fn blink(cx: blink::Context) {

  //   let high = cx.resources.led.is_set_high().unwrap();
  //   if high {
  //     cx.resources.led.set_low().unwrap();
  //   } else {
  //     cx.resources.led.set_high().unwrap();
  //   }

  //   cx.schedule.blink(cx.scheduled + CPU_FREQ.cycles()).unwrap();
  // }
}

