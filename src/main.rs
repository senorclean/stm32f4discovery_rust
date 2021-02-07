// #![deny(unsafe_code)]
#![no_main]
#![no_std]

#[allow(unused_extern_crates)]

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
  pwm::PwmChannels,
  pwm::C2,
  stm32::TIM4,
  // delay::Delay,
};



mod hb;

use rtic::cyccnt::{Instant, U32Ext};

use cortex_m_semihosting::hprintln;
use panic_semihosting as _;

const CPU_FREQ: u32 = 168_000_000;
const SAMPLE_SIZE: usize = 10;
const SAMPLE_THRESHOLD: usize = 5;

pub struct HeartBeatStatus {
  enabled: bool
}

pub struct ButtonData {
  cnt: usize,
  data: [bool; SAMPLE_SIZE] 
}

const fn convert_us_to_cycles(us: u32) -> u32 {
  us * (CPU_FREQ / 1_000_000)
}

fn is_debugger_connected() -> bool {
  let addr = 0xE000EDF0usize;
  let r = addr as *const u32;
  if unsafe{*r & 1} == 1 {
    true
  } else {
    false
  }
}

#[rtic::app(
  device = stm32f4xx_hal::stm32,
  peripherals = true,
  monotonic = rtic::cyccnt::CYCCNT
)]
const APP: () = {
  struct Resources {
    // led: gpiod::PD12<Output<PushPull>>,
    heartbeat_led: PwmChannels<TIM4, C2>,
    heartbeat_status: HeartBeatStatus,
    button: gpioa::PA0<Input<Floating>>,
    exti: stm32::EXTI,
    button_data: ButtonData,
    debugger: bool,
  }

  #[init(spawn = [heartbeat_mb], schedule = [heartbeat, button])]
  fn init(cx: init::Context) -> init::LateResources {

    let debugger = is_debugger_connected();
    if debugger {
      hprintln!("init").unwrap();
    };

    // device specific peripherals
    let device: stm32::Peripherals = cx.device;
    let mut syscfg = device.SYSCFG;
    let mut exti = device.EXTI;

    let gpioa = device.GPIOA.split();
    let gpiod = device.GPIOD.split();
    // let led = gpiod.pd12.into_push_pull_output();
    let mut button = gpioa.pa0.into_floating_input();

    let rcc = device.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(168.mhz()).freeze();

    // configure button as interrupt source
    button.make_interrupt_source(&mut syscfg);
    button.trigger_on_edge(&mut exti, Edge::RISING);
    button.enable_interrupt(&mut exti);

    // configure PWM module
    let pwm_channel = gpiod.pd13.into_alternate_af2();
    let mut heartbeat_led= pwm::tim4(device.TIM4, pwm_channel, clocks, 20u32.khz());
    heartbeat_led.set_duty(0);

    // start tasks
    // cx.schedule.blink(cx.start + 1_000.cycles()).unwrap();
    // cx.schedule.heartbeat(cx.start + 1_000.cycles(), true).unwrap();
    cx.spawn.heartbeat_mb(hb::Messages::TurnOn).unwrap();

    let heartbeat_status = HeartBeatStatus {
      enabled: false
    };

    let button_data = ButtonData {
      cnt: 0,
      data: [false; SAMPLE_SIZE]
    };

    init::LateResources {
      // led,
      heartbeat_led,
      heartbeat_status,
      button,
      exti,
      button_data,
      debugger
    }
  }

  #[task(binds = EXTI0, schedule = [button], resources = [button, exti])]
  fn exti0(cx: exti0::Context) {
    match cx.schedule.button(Instant::now() + convert_us_to_cycles(10_000).cycles()) {
      Ok(_) => (),
      // function is likely already scheduled
      Err(_) => ()
    }
    
    cx.resources.button.clear_interrupt_pending_bit();
    cx.resources.button.disable_interrupt(cx.resources.exti);
  }

  #[task(spawn = [heartbeat_mb], schedule = [button], resources = [button, exti, button_data, debugger])]
  fn button(cx: button::Context) {

    let cnt = &mut cx.resources.button_data.cnt;
    let data = &mut cx.resources.button_data.data;

    if *cnt < SAMPLE_SIZE {
      // continue scheduling itself recursively while sampling the button pin
      data[*cnt] = cx.resources.button.is_high().unwrap();
      *cnt += 1;
      cx.schedule.button(Instant::now() + convert_us_to_cycles(10_000).cycles()).unwrap();
    } else {
      // check to see if we have enough correct values to trigger a button press
      let sample_cnt = data.iter()
        .filter(|&x| *x == true)
        .count();

      // if *cx.resources.debugger {
      //   hprintln!("Count: {:?}", cnt).unwrap();
      // }

      if sample_cnt > SAMPLE_THRESHOLD {
        cx.spawn.heartbeat_mb(hb::Messages::Toggle).unwrap();
      }

      *cnt = 0;
      cx.resources.button.enable_interrupt(cx.resources.exti);
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

  #[task(priority = 2, schedule = [heartbeat], resources = [heartbeat_status, debugger])]
  fn heartbeat_mb(cx: heartbeat_mb::Context, msg: hb::Messages) {

    // check to see if we should do anything based on message
    match msg {
      hb::Messages::TurnOff => cx.resources.heartbeat_status.enabled = false,
      hb::Messages::TurnOn => {
        if !cx.resources.heartbeat_status.enabled {
          cx.resources.heartbeat_status.enabled = true;

          match cx.schedule.heartbeat(Instant::now(), true) {
            Ok(_) => (),
            Err(e) => {
              // function is likely already scheduled
              if *cx.resources.debugger {
                hprintln!("Heartbeat already scheduled. Error {:?}", e).unwrap();
              }
            }
          }
        }
      }
      hb::Messages::Toggle => {
        if !cx.resources.heartbeat_status.enabled {
          cx.resources.heartbeat_status.enabled = true;

          match cx.schedule.heartbeat(Instant::now(), true) {
            Ok(_) => (),
            Err(e) => {
              // function is likely already scheduled
              if *cx.resources.debugger {
                hprintln!("Heartbeat already scheduled. Error {:?}", e).unwrap();
              }
            }
          }
        } else {
          cx.resources.heartbeat_status.enabled = false;
        }
      }
    }
  }

  #[task(schedule = [heartbeat], resources = [heartbeat_led, heartbeat_status])]
  fn heartbeat(cx: heartbeat::Context, mut increment: bool) {

    hb::heartbeat(cx.resources.heartbeat_led, &mut increment);

    let led = cx.resources.heartbeat_led;
    let mut status = cx.resources.heartbeat_status;
    let schedule = cx.schedule;
    let scheduled = cx.scheduled;

    status.lock(|status| { 
      if status.enabled {
        schedule.heartbeat(scheduled + convert_us_to_cycles(30_000).cycles(), increment).unwrap();
      } else {
        led.disable();
        led.set_duty(0);
      }
    });
  }

  #[idle()]
  fn idle(_cx: idle::Context) -> ! {

    loop {
      // sleep while waiting for next event
      asm::wfi();
    }
  }

  extern "C" {
    fn SDIO();
    fn CRYP();
  }
};

