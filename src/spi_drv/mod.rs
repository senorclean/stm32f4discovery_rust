pub mod spi1;

pub const BUFFER_SIZE: usize = 255;

#[derive(Debug, Clone, Copy)]
pub struct Read {
  pub reg: u8,
  pub len: u8
}

#[derive(Debug, Clone, Copy)]
pub struct Write {
  pub reg: u8,
  pub len: u8,
  pub data: [u8; BUFFER_SIZE]
}

#[derive(Debug)]
pub enum Message {
  Ignore,
  StartRead(Read),
  StartWrite(Write),
  TxEvent,
  RxEvent,
  Error,
  EndTransaction,
  ReadConfirmation
}

#[derive(Debug)]
pub enum Action {
  DoNothing,
  Reject,
  StartRead(Read),
  StartWrite(Write),
  ContinueRead,
  ContinueWrite,
  ResetTransaction
}