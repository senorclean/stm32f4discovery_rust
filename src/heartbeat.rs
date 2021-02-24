use stm32f4xx_hal::{
  prelude::*,
  pwm::PwmChannels,
  pwm::C2,
  stm32::TIM4,
};

use rtic::cyccnt::{Instant, U32Ext};
use rtic_core::prelude::*;

use crate::util;
use crate::util::debugger;
use crate::app;
use crate::app::{
  heartbeat_mb_app,
  heartbeat_app,
  MessagePacket,
};


pub struct Data<T, U> {
  led: PwmChannels<T, U>,
  state: State,
}

#[derive(Debug)]
pub enum Message {
  TurnOff,
  TurnOn,
  Toggle
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum State {
  Off,
  On,
}

enum Action {
  DoNothing,
  Schedule
}


pub fn heartbeat_mb(cx: heartbeat_mb_app::Context, msg: MessagePacket) {

  let mut hb_data = cx.resources.heartbeat;

  (hb_data).lock(|hb_data| {

    match msg.msg { 
      app::Message::Heartbeat(x) => {
        
        let action;
        (hb_data.state, action) = hb_data.state.next(&x);

        match action {
          Action::Schedule => {
            match heartbeat_app::schedule(Instant::now(), true) {
              Ok(_) => (),
              Err(_) => {
                debugger::print(format_args!("Heartbeat is already scheduled"));
              }
            }
          }
          _ => ()
        }
      }
      _ => ()
    }
  });
}

pub fn heartbeat(cx: heartbeat_app::Context, mut increment: bool) {

  let mut hb_data = cx.resources.heartbeat;
  let scheduled = cx.scheduled;

  (hb_data).lock(|hb_data| {
    adjust_duty_cycle(&mut hb_data.led, &mut increment);

    if hb_data.state == State::On {
      heartbeat_app::schedule(scheduled + util::convert_us_to_cycles(30_000).cycles(), increment).unwrap();
    } else {
      hb_data.led.disable();
      hb_data.led.set_duty(0);
    }
  });
}

fn adjust_duty_cycle(led: &mut PwmChannels<TIM4, C2>, increment: &mut bool) {
  // max duty is 4200 so 4200 / 100 = 42 total steps
  const STEP_SIZE: u16 = 100;

  let curr_duty = led.get_duty();

  // see if direction should change
  if curr_duty == 0 {
    *increment = true;
  } else if curr_duty == led.get_max_duty() {
    *increment = false;
  }

  led.disable();
  if *increment {
    led.set_duty(curr_duty + STEP_SIZE);
  } else {
    led.set_duty(curr_duty - STEP_SIZE);
  }
  led.enable();
}


impl State {
  pub fn next(self, msg: &Message) -> (State, Action) {
    match (self, msg) {
      (State::Off, Message::TurnOn) => {
        (State::On, Action::Schedule)
      }
      (State::Off, Message::Toggle) => {
        (State::On, Action::Schedule)
      }
      (State::On, Message::TurnOff) => {
        (State::Off, Action::DoNothing)
      }
      (State::On, Message::Toggle) => {
        (State::Off, Action::DoNothing)
      }
      (s, _m) => {
        (s, Action::DoNothing)
      }
    }
  }
}

impl<T, U> Data<T, U> {
  pub fn new(led: PwmChannels<T, U>) -> Self {
    Data {
      led,
      state: State::Off
    } 
  }
}