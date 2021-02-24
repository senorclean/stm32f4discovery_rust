use crate::constants;
use crate::app::{
  lis_mb_app,
  heartbeat_mb_app,
  spi1_mb_app,
  button_mb_app,
  Task,
  MessagePacket
};
use rtic::cyccnt::{Instant, U32Ext};
use crate::app;

pub const fn convert_us_to_cycles(us: u32) -> u32 {
  us * (constants::CPU_FREQ / 1_000_000)
}

pub mod debugger {
  use cortex_m_semihosting::hprintln;

  static mut ENABLED: bool = false;

  pub fn init() {
    let addr = 0xE000EDF0usize;
    let r = addr as *const u32;
    if unsafe { *r & 1 } == 1 {
      unsafe { ENABLED = true; }
    }
  }

  pub fn print(s: core::fmt::Arguments) {
    if unsafe { ENABLED } {
      hprintln!("{}", s).unwrap();
    }
  }
}

pub fn send_message(source: Task, dest: &Task, msg: app::Message) -> Result<(), RticError> {
  match dest {
    Task::Lis3dsh => {
      lis_mb_app::spawn(MessagePacket {
        source,
        msg
      }).map_err(|e| RticError::Spawn(e))?;
    }
    Task::Heartbeat => {
      heartbeat_mb_app::spawn(MessagePacket {
        source,
        msg
      }).map_err(|e| RticError::Spawn(e))?;
    }
    Task::Button => {
      button_mb_app::spawn(MessagePacket {
        source,
        msg
      }).map_err(|e| RticError::Spawn(e))?;
    }
    Task::Spi1 => {
      spi1_mb_app::spawn(MessagePacket {
        source,
        msg
      }).map_err(|e| RticError::Spawn(e))?;
    }
    Task::Init => (),
    Task::Interrupt => (),
  }

  Ok(())
}

pub fn schedule_message(source: Task, dest: &Task, msg: app::Message, micros_from_now: u32) -> Result<(), RticError> {
  let sched_time = Instant::now() + convert_us_to_cycles(micros_from_now).cycles();

  match dest {
    Task::Lis3dsh => {
      lis_mb_app::schedule(sched_time, MessagePacket {
        source,
        msg
      }).map_err(|e| RticError::Schedule(e))?;
    }
    Task::Heartbeat => {
      heartbeat_mb_app::schedule(sched_time, MessagePacket {
        source,
        msg
      }).map_err(|e| RticError::Schedule(e))?;
    }
    Task::Button => {
      button_mb_app::schedule(sched_time, MessagePacket {
        source,
        msg
      }).map_err(|e| RticError::Schedule(e))?;
    }
    Task::Spi1 => {
      spi1_mb_app::schedule(sched_time, MessagePacket {
        source,
        msg
      }).map_err(|e| RticError::Schedule(e))?;
    }
    Task::Init => (),
    Task::Interrupt => (),
  }

  Ok(())
}

#[derive(Debug)]
pub enum RticError {
  Spawn(app::MessagePacket),
  Schedule(app::MessagePacket),
}
