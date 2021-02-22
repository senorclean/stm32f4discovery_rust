use rtic_core::prelude::*;
use stm32f4xx_hal::{
  prelude::*,
  gpio::ExtiPin,
  // delay::Delay,
};
use rtic::cyccnt::{Instant, U32Ext};

use crate::util;
use crate::util::debugger;
use crate::heartbeat;
use crate::app;
use crate::app::{
  button_mb_app,
  button_app,
  MessagePacket,
  Task,
};

pub const SAMPLE_SIZE: usize = 10;
const SAMPLE_THRESHOLD: usize = 5;

#[derive(Debug)]
pub enum Message {
  ButtonPressed,
  ButtonNotPressed
}

pub struct Data<T> {
  pub button: T,
  state: State,
  sample_cnt: usize,
  sample_data: [bool; SAMPLE_SIZE] 
}

enum Action {
  DoNothing,
  Schedule
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum State {
  NotPressed,
  Pressed,
}


pub fn button_mb(cx: button_mb_app::Context, msg: MessagePacket) {

  let mut button_data = cx.resources.button;

  (button_data).lock(|button_data| {

    match msg.msg { 
      app::Message::Button(x) => {
        
        let action;
        (button_data.state, action) = button_data.state.next(&x);

        match action {
          Action::Schedule => {
            match button_app::schedule(Instant::now()) {
              Ok(_) => (),
              Err(_) => {
                debugger::print("Button is already scheduled", None);
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

pub fn button(cx: button_app::Context) {

    let button_data = cx.resources.button;
    let exti = cx.resources.exti;

    (button_data, exti).lock(|button_data, exti| {

      if button_data.sample_cnt < SAMPLE_SIZE {
        // continue scheduling itself recursively while sampling the button pin
        button_data.sample_data[button_data.sample_cnt] = button_data.button.is_high().unwrap();
        button_data.sample_cnt += 1;
        button_app::schedule(Instant::now() + util::convert_us_to_cycles(10_000).cycles()).unwrap();
      } else {
        // check to see if we have enough correct values to trigger a button press
        let sample_cnt = button_data.sample_data.iter()
          .filter(|&x| *x == true)
          .count();

        // if *cx.resources.debugger {
        //   hprintln!("Count: {:?}", cnt).unwrap();
        // }

        if sample_cnt > SAMPLE_THRESHOLD {
          util::send_message(Task::Spi1, &Task::Heartbeat, app::Message::Heartbeat(heartbeat::Message::Toggle));
        }

        button_data.sample_cnt = 0;
        (button_data.state, ..) = button_data.state.next(&Message::ButtonNotPressed);
        button_data.button.enable_interrupt(exti);
      }
    });
  }


impl State {
  pub fn next(self, msg: &Message) -> (State, Action) {
    match (self, msg) {
      (State::NotPressed, Message::ButtonPressed) => {
        (State::Pressed, Action::Schedule)
      }
      (State::Pressed, Message::ButtonNotPressed) => {
        (State::NotPressed, Action::DoNothing)
      }
      (s, _m) => {
        (s, Action::DoNothing)
      }
    }
  }
}

impl<T> Data<T> {
  pub fn new(button: T) -> Self {
    Data {
      button,
      state: State::NotPressed,
      sample_cnt: 0,
      sample_data: [false; SAMPLE_SIZE]
    }
  }
}