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
  TX_BUFFER_SIZE,
  RX_BUFFER_SIZE,
};
use crate::app;
use crate::app::{
  spi1_mb_app,
  MessagePacket,
  Task,
};

pub struct Data<T, U> {
  pub spi: T,
  cs_pin: U,
  state: State,
  origin: Task,
  tx_buffer: [u8; TX_BUFFER_SIZE],
  pub rx_buffer: [u8; RX_BUFFER_SIZE],
  transfer_len: u8,
  bytes_transferred: u8
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
          Action::StartRead(r) => {
            spi.origin = msg.source;

            spi.spi.listen(spi::Event::Rxne);
            spi.cs_pin.set_low().unwrap();
            spi.tx_buffer[0] = r.reg;
            spi.transfer_len = r.len;
            spi.spi.send(spi.tx_buffer[0]).unwrap();
          }
          Action::StartWrite(w) => {
            spi.origin = msg.source;

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
          }
          Action::ContinueRead => {
            let val = spi.spi.read().unwrap();

            // the first byte is garbage data
            if spi.bytes_transferred > 0 {
              spi.rx_buffer[(usize::from(spi.bytes_transferred) - 1)] = val;
            }

            if spi.bytes_transferred == spi.transfer_len {
              (spi.state, ..) = spi.state.next(&Message::FinishTransaction);

              spi.cs_pin.set_high().unwrap();

              util::send_message(Task::Spi1, &spi.origin, app::Message::Lis3dsh(lis3dsh::Message::ReadComplete)).unwrap();

              // reset transaction
              spi.transfer_len = 0;
              spi.bytes_transferred = 0;
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

              (spi.state, ..) = spi.state.next(&Message::FinishTransaction);
              spi.cs_pin.set_high().unwrap();

              // spawn message to origin
              util::send_message(Task::Spi1, &spi.origin, app::Message::Lis3dsh(lis3dsh::Message::WriteComplete)).unwrap();
              
              // reset transaction
              spi.transfer_len = 0;
              spi.bytes_transferred = 0;
              
            } else {
              spi.spi.listen(spi::Event::Rxne);
              spi.spi.send(spi.tx_buffer[usize::from(spi.bytes_transferred)]).unwrap();
            }
          }
          Action::Reject => {
            match spi.origin {
              Task::Lis3dsh => {
                util::send_message(Task::Spi1, &spi.origin, app::Message::Lis3dsh(lis3dsh::Message::CommandRejected)).unwrap();
              }
              _ => ()
            }
          }
          Action::Reset => {
            // reset variables
            spi.transfer_len = 0;
            spi.bytes_transferred = 0;
          }
          Action::DoNothing => (),
        }
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
      (State::Reading, Message::FinishTransaction) => {
        (State::WaitingForConfirmation, Action::DoNothing)
      }
      (State::Writing, Message::FinishTransaction) => {
        (State::WaitingForConfirmation, Action::DoNothing)
      }
      (State::WaitingForConfirmation, Message::ReadConfirmation) => {
        (State::Idling, Action::DoNothing)
      }
      (State::WaitingForConfirmation, Message::WriteConfirmation) => {
        (State::Idling, Action::DoNothing)
      }
      (_s, Message::CancelTransaction) => {
        (State::Idling, Action::Reset)
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
      tx_buffer: [0; TX_BUFFER_SIZE],
      rx_buffer: [0; RX_BUFFER_SIZE],
      transfer_len: 0,
      bytes_transferred: 0
    }
  }
}