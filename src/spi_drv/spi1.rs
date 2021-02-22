use rtic_core::prelude::*;
use stm32f4xx_hal::{
  prelude::*,
  spi,
};

use crate::lis3dsh;
use crate::util;
use crate::spi_drv::{
  Message,
  Action,
  BUFFER_SIZE
};
use crate::app;
use crate::app::{
  spi1_mb_app,
  spi1_app,
  MessagePacket,
  Task,
};

pub struct Data<T, U> {
  pub spi: T,
  cs_pin: U,
  state: State,
  origin: Task,
  tx_buffer: [u8; BUFFER_SIZE],
  pub rx_buffer: [u8; BUFFER_SIZE],
  transfer_len: u8,
  pub bytes_transferred: u8
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum State {
  Idling,
  Reading,
  Writing,
  WaitingForConfirmation,
}

pub fn spi1_mb(mut cx: spi1_mb_app::Context, msg: MessagePacket) {
  (cx.resources.spi).lock(|spi| {

    match msg.msg { 
      app::Message::Spi(x) => {

        let action;
        (spi.state, action) = spi.state.next(&x);

        match action {
          Action::StartRead(_) => {
            spi.origin = msg.source;
          }
          Action::StartWrite(_) => {
            spi.origin = msg.source;
          }
          Action::ContinueRead => (),
          Action::ContinueWrite => (),
          Action::ResetTransaction => (),
          Action::Reject => {
            util::send_message(Task::Spi1, &spi.origin, app::Message::Lis3dsh(lis3dsh::Message::CommandRejected));
            return
          }
          Action::DoNothing => return,
        }

        spi1_app::spawn(action).unwrap();
      }
      _ => return
    }
  });
}

pub fn spi1(mut cx: spi1_app::Context, msg: Action) {
  (cx.resources.spi).lock(|spi| {
    match msg {
      Action::StartRead(r) => {
        spi.spi.listen(spi::Event::Rxne);
        spi.cs_pin.set_low().unwrap();
        // spi.spi.send(spi.tx_buffer[0]).unwrap();
        spi.tx_buffer[0] = r.reg;
        spi.transfer_len = r.len;
        spi.spi.send(spi.tx_buffer[0]).unwrap();
      },
      Action::StartWrite(w) => {
        spi.spi.listen(spi::Event::Rxne);
        spi.cs_pin.set_low().unwrap();

        spi.tx_buffer[0] = w.reg;
        spi.transfer_len = w.len;

        let mut i: usize = 0;
        while i < w.len.into() {
          spi.tx_buffer[i + 1] = w.data[i];
          i += 1;
        }

        spi.spi.send(spi.tx_buffer[0]).unwrap();
      },
      Action::ContinueRead => {
        let val = spi.spi.read().unwrap();

        // the first byte is garbage data
        if spi.bytes_transferred > 0 {
          spi.rx_buffer[(usize::from(spi.bytes_transferred) - 1)] = val;
        }

        if spi.bytes_transferred == spi.transfer_len {
          (spi.state, ..) = spi.state.next(&Message::EndTransaction);

          spi.cs_pin.set_high().unwrap();

          util::send_message(Task::Spi1, &spi.origin, app::Message::Lis3dsh(lis3dsh::Message::ReadComplete));

          // debugger::print("Final value: ", Some(val.into()));
        } else {
          // send 0x00 as dummy byte to keep transfer going
          spi.spi.listen(spi::Event::Rxne);
          spi.spi.send(0x00).unwrap();
          spi.bytes_transferred += 1;
        }
      },
      Action::ContinueWrite => {
        // read out the value from spi module and ignore it
        spi.spi.read().unwrap();
        spi.bytes_transferred += 1;

        if spi.bytes_transferred == spi.transfer_len {

          (spi.state, ..) = spi.state.next(&Message::EndTransaction);
          spi.cs_pin.set_high().unwrap();

          // spawn message to origin
          // let origin = &spi.origin;
        } else {
          spi.spi.listen(spi::Event::Rxne);
          spi.spi.send(spi.tx_buffer[usize::from(spi.bytes_transferred)]).unwrap();
        }
      },
      Action::ResetTransaction => {
        spi.bytes_transferred = 0;
        spi.transfer_len = 0;
      }
      _ => ()
    }
  });
}


impl State {
  pub fn next(self, msg: &Message) -> (State, Action) {
    match (self, msg) {
      (State::Idling, Message::StartRead(r)) => {
        (State::Reading, Action::StartRead(*r))
      }
      (State::Idling, Message::StartWrite(w)) => {
        (State::Writing, Action::StartWrite(*w))
      }
      (State::Reading, Message::RxEvent) => {
        (State::Reading, Action::ContinueRead)
      }
      (State::Writing, Message::RxEvent) => {
        (State::Writing, Action::ContinueWrite)
      }
      (State::Reading, Message::EndTransaction) => {
        (State::WaitingForConfirmation, Action::DoNothing)
      }
      (State::Writing, Message::EndTransaction) => {
        (State::Idling, Action::DoNothing)
      }
      (State::WaitingForConfirmation, Message::ReadConfirmation) => {
        (State::Idling, Action::ResetTransaction)
      }
      (s, _m) => {
        (s, Action::Reject)
      }
    }
  }
}

impl<T, U> Data<T, U> {
  pub fn new(spi: T, cs_pin: U) -> Self {
    Data {
      spi,
      cs_pin,
      state: State::Idling,
      origin: Task::Init,
      tx_buffer: [0; BUFFER_SIZE],
      rx_buffer: [0; BUFFER_SIZE],
      transfer_len: 0,
      bytes_transferred: 0
    }
  }
}