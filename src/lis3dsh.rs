use rtic_core::prelude::*;
use rtic::cyccnt::{Instant, U32Ext};

const READ_MASK: u8 = 0x80;
const WRITE_MASK: u8 = 0x00;

use crate::util;
use crate::util::debugger;
use crate::spi_drv;
use crate::app;
use crate::app::{
  lis_mb_app,
  lis_app,
  spi1_mb_app,
  MessagePacket,
  Task,
};

#[derive(Debug)]
pub enum Message {
  Read(spi_drv::Read),
  Write(spi_drv::Write),
  ReadComplete,
  WriteComplete,
  CommandRejected,
}

#[derive(Debug)]
pub struct ReadRegister;

impl ReadRegister {
  pub const ID: u8 = (READ_MASK | 0x0F);
}


#[derive(Debug)]
pub enum WriteRegister {
}

pub struct Lis3dsh {
  state: State,
  origin: Task,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
  DoNothing,
  StartRead(spi_drv::Read),
  StartWrite(spi_drv::Write),
  HandleData,
  LogError,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum State {
  Idling,
  Reading,
  Writing,
}

pub fn lis3dsh_mb(mut cx: lis_mb_app::Context, msg: MessagePacket) {
  (cx.resources.lis).lock(|lis| {

    match msg.msg { 
      app::Message::Lis3dsh(x) => {
        
        let action;
        (lis.state, action) = lis.state.next(&x);

        match action {
          Action::StartRead(reg) => {
            lis_app::spawn(Action::StartRead(reg)).unwrap();
          }
          Action::StartWrite(reg) => {
            lis_app::spawn(Action::StartWrite(reg)).unwrap();
          }
          Action::HandleData => {
            lis_app::spawn(Action::HandleData).unwrap();
          }
          Action::LogError => {
            debugger::print("Command rejected from task: ", Some(u32::from(msg.source)));
          }
          Action::DoNothing => return,
        }
      }
      _ => ()
    }
  });
}

pub fn lis3dsh(mut cx: lis_app::Context, msg: Action) {
  (cx.resources.spi).lock(|spi| {
    match msg {
      Action::StartRead(reg) => {
        spi1_mb_app::spawn(MessagePacket {
          source: Task::Lis3dsh,
          msg: app::Message::Spi(spi_drv::Message::StartRead(reg))
        }).unwrap();
      }
      Action::StartWrite(reg) => {
        spi1_mb_app::spawn(MessagePacket {
          source: Task::Lis3dsh,
          msg: app::Message::Spi(spi_drv::Message::StartWrite(reg))
        }).unwrap();
      }
      Action::HandleData => {
        // just print out everything for now
        let mut i: usize = 0;
        while i < spi.bytes_transferred.into() {
          debugger::print("Received value: ", Some(spi.rx_buffer[i].into()));
          i += 1;
        }

        // confirm the value to unblock the spi module
        let msg = app::Message::Spi(spi_drv::Message::ReadConfirmation);
        util::send_message(Task::Lis3dsh, &Task::Spi1, msg);

        let msg = app::Message::Lis3dsh(Message::Read(spi_drv::Read { reg: ReadRegister::ID, len: 1 }));
        util::schedule_message(Task::Lis3dsh, &Task::Lis3dsh, msg, 1_000_000);
      }
      Action::DoNothing => (),
      Action::LogError => (),
    }
  });
}


impl State {
  pub fn next(self, msg: &Message) -> (State, Action) {
    match (self, msg) {
      (State::Idling, Message::Read(reg)) => {
        (State::Reading, Action::StartRead(*reg))
      }
      (State::Idling, Message::Write(reg)) => {
        (State::Writing, Action::StartWrite(*reg))
      }
      (State::Reading, Message::ReadComplete) => {
        (State::Idling, Action::HandleData)
      }
      (State::Writing, Message::WriteComplete) => {
        (State::Reading, Action::DoNothing)
      }
      (s, Message::CommandRejected) => {
        (s, Action::LogError)
      }
      (s, _m) => {
        (s, Action::DoNothing)
      }
    }
  }
}

impl Lis3dsh {
  pub fn new() -> Self {
    Lis3dsh {
      state: State::Idling,
      origin: Task::Init,
    }
  }
}