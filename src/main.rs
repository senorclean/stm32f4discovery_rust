// #![deny(unsafe_code)]
#![no_main]
#![no_std]
#![feature(destructuring_assignment)]

#[allow(unused_extern_crates)]

mod heartbeat;
mod util;
mod constants;
mod button;
mod spi_drv;
mod lis3dsh;
// mod i2c_drv;

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
    spi,
    gpio::gpioa,
    gpio::gpioe,
    // gpio::gpiod,
    gpio::gpioa::PA5,
    gpio::gpioa::PA6,
    gpio::gpioa::PA7,
    gpio::AF5,
    gpio::Output,
    gpio::Input,
    gpio::Alternate,
    gpio::Floating,
    gpio::Edge,
    gpio::PushPull,
    gpio::ExtiPin,
    pwm::C2,
    stm32::TIM4,
    stm32::SPI1,
  };
  use rtic_core::prelude::*;

  use panic_semihosting as _;

  #[resources]
  struct Resources {
    heartbeat: heartbeat::Data<TIM4, C2>,
    button: button::Data<gpioa::PA0<Input<Floating>>>,
    spi: spi_drv::spi1::Data<spi::Spi<SPI1, (PA5<Alternate<AF5>>, PA6<Alternate<AF5>>, PA7<Alternate<AF5>>)>, gpioe::PE3<Output<PushPull>>>,
    lis: lis3dsh::Lis3dsh,
    exti: stm32::EXTI,
  }

  #[init()]
  fn init(cx: init::Context) -> init::LateResources {

    util::debugger::init();
    util::debugger::print(format_args!("Initializing"));

    // device specific peripherals
    let device: stm32::Peripherals = cx.device;
    let mut syscfg = device.SYSCFG;
    let mut exti = device.EXTI;

    let rcc = device.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(168.mhz()).freeze();

    let gpioa = device.GPIOA.split();
    let gpiod = device.GPIOD.split();
    let gpioe = device.GPIOE.split();

    // configure button as interrupt source
    let mut button = gpioa.pa0.into_floating_input();
    button.make_interrupt_source(&mut syscfg);
    button.trigger_on_edge(&mut exti, Edge::RISING);
    button.enable_interrupt(&mut exti);

    // configure PWM module
    let pwm_channel = gpiod.pd13.into_alternate_af2();
    let mut heartbeat_led= pwm::tim4(device.TIM4, pwm_channel, clocks, 20u32.khz());
    heartbeat_led.set_duty(0);

    // setup SPI1 for accelerometer
    let spi_clk = gpioa.pa5.into_alternate_af5();
    let spi_miso = gpioa.pa6.into_alternate_af5();
    let spi_mosi = gpioa.pa7.into_alternate_af5();
    let mut spi_cs = gpioe.pe3.into_push_pull_output();
    let mode = spi::Mode {
      polarity: spi::Polarity::IdleLow,
      phase: spi::Phase::CaptureOnFirstTransition
    };

    let mut spi1 = spi::Spi::spi1(device.SPI1,
      (spi_clk, spi_miso, spi_mosi),
      mode,
      1u32.mhz().into(),
      clocks);
    
    spi1.listen(spi::Event::Error);
    spi_cs.set_high().unwrap();

    // initialize resource data
    let button = button::Data::new(button);
    let heartbeat = heartbeat::Data::new(heartbeat_led);
    let lis = lis3dsh::Lis3dsh::new();
    let spi = spi_drv::spi1::Data::new(spi1, spi_cs);

    // start tasks
    let msg = Message::Heartbeat(heartbeat::Message::TurnOn);
    util::send_message(Task::Init, &Task::Heartbeat, msg).unwrap();

    let msg = Message::Lis3dsh(lis3dsh::Message::ChangeDataRate(lis3dsh::DataRate::OneHundredHertz));
    util::send_message(Task::Init, &Task::Lis3dsh, msg).unwrap();

    let msg = Message::Lis3dsh(lis3dsh::Message::ReadAxes);
    util::schedule_message(Task::Init, &Task::Lis3dsh, msg, 1_000_000).unwrap();

    init::LateResources {
      heartbeat,
      button,
      lis,
      exti,
      spi,
    }
  }

  #[task(priority = 3, binds = EXTI0, resources = [button, exti])]
  fn exti0(cx: exti0::Context) {

    util::send_message(Task::Interrupt, &Task::Button, Message::Button(button::Message::ButtonPressed)).unwrap();

    let exti = cx.resources.exti;
    let button = cx.resources.button;
    
    (button, exti).lock(|button, exti| {
        button.button.clear_interrupt_pending_bit();
        button.button.disable_interrupt(exti);
    });
  }

  #[task(priority = 3, binds = SPI1, resources = [spi])]
  fn spi1(mut cx: spi1::Context) {
    let mut msg = Message::Spi(spi_drv::Message::Ignore);

    (cx.resources.spi).lock(|spi| {
      if spi.spi.is_rxne() {
        // util::debugger::print(format_args!("RX not empty event"));
        msg = Message::Spi(spi_drv::Message::RxEvent);
        spi.spi.unlisten(spi::Event::Rxne);
      } else if spi.spi.is_txe() {
        // util::debugger::print(format_args!("TX empty event"));
        msg = Message::Spi(spi_drv::Message::TxEvent);
      } else {
        // util::debugger::print(format_args!("Unknown event received"));
      }
    });

    util::send_message(Task::Interrupt, &Task::Spi1, msg).unwrap();
  }

  #[task(priority = 2, resources = [lis, spi], capacity = 4)]
  fn lis_mb_app(cx: lis_mb_app::Context, msg: MessagePacket) {
    lis3dsh::lis3dsh_mb(cx, msg);
  }

  #[task(priority = 2, resources = [spi], capacity = 2)]
  fn spi1_mb_app(cx: spi1_mb_app::Context, msg: MessagePacket) {
    spi_drv::spi1::spi1_mb(cx, msg);
  }

  #[task(priority = 2, resources = [button])]
  fn button_mb_app(cx: button_mb_app::Context, msg: MessagePacket) {
    button::button_mb(cx, msg);
  }

  #[task(resources = [button, exti])]
  fn button_app(cx: button_app::Context) {
    button::button(cx);
  }

  #[task(priority = 2, resources = [heartbeat])]
  fn heartbeat_mb_app(cx: heartbeat_mb_app::Context, msg: MessagePacket) {
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

  #[derive(Debug)]
  pub enum Task {
    Init,
    Interrupt,
    Heartbeat,
    Button,
    Spi1,
    Lis3dsh
  }

  #[derive(Debug)]
  pub enum Message {
    Lis3dsh(lis3dsh::Message),
    Heartbeat(heartbeat::Message),
    Button(button::Message),
    Spi(spi_drv::Message),
  }

  #[derive(Debug)]
  pub struct MessagePacket {
    pub source: Task,
    pub msg: Message
  }
}
