use rtic_core::prelude::*;
use rtic::cyccnt::{Instant, U32Ext};

const TIMEOUT: u32 = 3_000_000;
const READ_MASK: u8 = 0x80;
const WRITE_MASK: u8 = 0x00;

const SCALE_BYTE_POS: u8 = 3;
const DATA_RATE_BYTE_POS: u8 = 4;
const BDU_BYTE_POS: u8 = 3;
const Z_EN_BYTE_POS: u8 = 2;
const Y_EN_BYTE_POS: u8 = 1;
const X_EN_BYTE_POS: u8 = 0;

use crate::util;
use crate::util::debugger;
use crate::spi_drv;
use crate::app;
use crate::app::{
  lis_mb_app,
  MessagePacket,
  Task,
};

#[derive(Debug, Clone, Copy)]
pub enum Scale {
  Two_G,
  Four_G,
  Six_G,
  Eight_G,
  Sixteen_G,
}

#[derive(Debug, Clone, Copy)]
pub enum DataRate {
  Zero,
  OneHundredHertz
}

#[derive(Debug)]
pub enum Message {
  ReadID,
  ReadAxes,
  ChangeScale(Scale),
  ChangeDataRate(DataRate),
  ChangeBDU(bool),
  ReadComplete,
  WriteComplete,
  CommandRejected,
  TimeoutCheck,
}

#[derive(Debug)]
pub struct ReadRegister;

impl ReadRegister {
  pub const ID: u8 = (READ_MASK | 0x0F);
  pub const X_AXIS: u8 = (READ_MASK | 0x28);
  pub const Y_AXIS: u8 = (READ_MASK | 0x2A);
  pub const Z_AXIS: u8 = (READ_MASK | 0x2C);
}

#[derive(Debug)]
pub struct WriteRegister;

impl WriteRegister {
  pub const CTRL_REG4: u8 = (WRITE_MASK | 0x20);
  pub const CTRL_REG5: u8 = (WRITE_MASK | 0x24);
}

struct Configuration {
  x_en: u8,
  y_en: u8,
  z_en: u8,
  // block data update
  bdu: u8,
  scale: Scale,
  data_rate: DataRate
}

pub struct Lis3dsh {
  state: State,
  config: Configuration,
  origin: Task,
  current_process: Message,
}

#[derive(Debug)]
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
  Busy,
}

pub fn lis3dsh_mb(mut cx: lis_mb_app::Context, packet: MessagePacket) {
  (cx.resources.lis, cx.resources.spi).lock(|lis, spi| {

    match packet.msg { 
      app::Message::Lis3dsh(msg) => {
        
        let action;
        (lis.state, action) = lis.state.next(&msg, lis);

        match action {
          Action::StartRead(reg) => {
            lis.origin = packet.source;
            lis.current_process = msg;
            let msg = app::Message::Spi(spi_drv::Message::StartRead(reg));
            util::send_message(Task::Lis3dsh, &Task::Spi1, msg).unwrap();

            let msg = app::Message::Lis3dsh(Message::TimeoutCheck);
            util::schedule_message(Task::Lis3dsh, &Task::Lis3dsh, msg, TIMEOUT).unwrap();
          }
          Action::StartWrite(reg) => {
            lis.origin = packet.source;
            lis.current_process = msg;
            let msg = app::Message::Spi(spi_drv::Message::StartWrite(reg));
            util::send_message(Task::Lis3dsh, &Task::Spi1, msg).unwrap();

            let msg = app::Message::Lis3dsh(Message::TimeoutCheck);
            util::schedule_message(Task::Lis3dsh, &Task::Lis3dsh, msg, TIMEOUT).unwrap();
          }
          Action::HandleData => {
            match lis.current_process {
              Message::ReadID => {
                let res = spi.rx_buffer[0];
                
                // confirm the value to unblock the spi module
                let msg = app::Message::Spi(spi_drv::Message::ReadConfirmation);
                util::send_message(Task::Lis3dsh, &Task::Spi1, msg).unwrap();

                debugger::print(format_args!("Received value: {}", res));

                // respond to origin
              },
              Message::ReadAxes => {
                let x_axis = ((u16::from(spi.rx_buffer[1]) << 8) | u16::from(spi.rx_buffer[0])) as i16;
                let y_axis = ((u16::from(spi.rx_buffer[3]) << 8) | u16::from(spi.rx_buffer[2])) as i16;
                let z_axis = ((u16::from(spi.rx_buffer[5]) << 8) | u16::from(spi.rx_buffer[4])) as i16;

                // confirm the value to unblock the spi module
                let msg = app::Message::Spi(spi_drv::Message::ReadConfirmation);
                util::send_message(Task::Lis3dsh, &Task::Spi1, msg).unwrap();

                let x_axis = calculate_1g(x_axis, &lis.config.scale);
                let y_axis = calculate_1g(y_axis, &lis.config.scale);
                let z_axis = calculate_1g(z_axis, &lis.config.scale);

                debugger::print(format_args!("X-axis: {:?}", x_axis));
                debugger::print(format_args!("Y-axis: {:?}", y_axis));
                debugger::print(format_args!("Z-axis: {:?}", z_axis));

                // respond to origin
              }
              Message::ChangeScale(x) => {
                lis.config.scale = x;

                // confirm the value to unblock the spi module
                let msg = app::Message::Spi(spi_drv::Message::WriteConfirmation);
                util::send_message(Task::Lis3dsh, &Task::Spi1, msg).unwrap();
              }
              Message::ChangeDataRate(r) => {
                lis.config.data_rate = r;

                // confirm the value to unblock the spi module
                let msg = app::Message::Spi(spi_drv::Message::WriteConfirmation);
                util::send_message(Task::Lis3dsh, &Task::Spi1, msg).unwrap();
              }
              _ => ()
            }
          }
          Action::LogError => {
            debugger::print(format_args!("[Error: {:?}] Failed to perform task: {:?}", msg, lis.current_process));
          }
          Action::DoNothing => (),
        }
      }
      _ => ()
    }
  });
}

fn calculate_1g(input: i16, scale: &Scale) -> f32 {
  let scale: u8 = match scale {
    Scale::Two_G => 2,
    Scale::Four_G => 4,
    Scale::Six_G => 6,
    Scale::Eight_G => 8,
    Scale::Sixteen_G => 16,
  };

  (f32::from(input) * f32::from(scale)) / 32768.0
}


impl State {
  pub fn next(self, msg: &Message, lis: &Lis3dsh) -> (State, Action) {
    match (self, msg) {
      (State::Idling, Message::ReadID) => {
        (State::Busy, Action::StartRead(spi_drv::Read { reg: ReadRegister::ID, len: 1 }))
      }
      (State::Idling, Message::ReadAxes) => {
        // auto-increment is enabled by default so this will read out all the axis registers
        (State::Busy, Action::StartRead(spi_drv::Read { reg: ReadRegister::X_AXIS, len: 6 }))
      }
      (State::Idling, Message::ChangeScale(scale)) => {
        let scale = u8::from(*scale);
        let mut data = [0; 10];
        data[0] = scale << 3;

        (State::Busy, Action::StartWrite(spi_drv::Write { reg: WriteRegister::CTRL_REG5, len: 2, data}))
      }
      (State::Idling, Message::ChangeDataRate(rate)) => {
        let rate = u8::from(*rate);
        let mut data = [0; 10];
        // data[0] = (rate << 4) | 0x0F;
        data[0] = u8::from(rate) |
          lis.config.bdu |
          lis.config.z_en |
          lis.config.y_en |
          lis.config.x_en;

        (State::Busy, Action::StartWrite(spi_drv::Write { reg: WriteRegister::CTRL_REG4, len: 2, data}))
      }
      (State::Idling, Message::ChangeBDU(bdu)) => {
        let bdu = u8::from(*bdu);
        let mut data = [0; 10];

        data[0] = (bdu << BDU_BYTE_POS) |
          u8::from(lis.config.data_rate) |
          lis.config.z_en |
          lis.config.y_en |
          lis.config.x_en;

        (State::Busy, Action::StartWrite(spi_drv::Write { reg: WriteRegister::CTRL_REG4, len: 2, data}))
      }
      (State::Busy, Message::ReadComplete) => {
        (State::Idling, Action::HandleData)
      }
      (State::Busy, Message::WriteComplete) => {
        (State::Idling, Action::HandleData)
      }
      (State::Busy, Message::TimeoutCheck) => {
        (State::Idling, Action::LogError)
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
      config: Configuration::default(),
      origin: Task::Init,
      current_process: Message::CommandRejected
    }
  }
}

impl From<Scale> for u8 {
  fn from(x: Scale) -> Self {
    match x {
      Scale::Two_G => (0 << SCALE_BYTE_POS),
      Scale::Four_G => (1 << SCALE_BYTE_POS),
      Scale::Six_G => (2 << SCALE_BYTE_POS),
      Scale::Eight_G => (3 << SCALE_BYTE_POS),
      Scale::Sixteen_G => (4 << SCALE_BYTE_POS),
    }
  }
}

impl From<DataRate> for u8 {
  fn from(x: DataRate) -> Self {
    match x {
      DataRate::Zero => (0 << DATA_RATE_BYTE_POS),
      DataRate::OneHundredHertz => (6 << DATA_RATE_BYTE_POS)
    }
  }
}

impl Default for Configuration {
  fn default() -> Self {
    Configuration {
      x_en: (1 << X_EN_BYTE_POS),
      y_en: (1 << Y_EN_BYTE_POS),
      z_en: (1 << Z_EN_BYTE_POS),
      bdu: (0 << BDU_BYTE_POS),
      scale: Scale::Two_G,
      data_rate: DataRate::Zero
    }
  }
}
