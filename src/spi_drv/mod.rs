pub mod spi1;

pub const RX_BUFFER_SIZE: usize = 255;
pub const TX_BUFFER_SIZE: usize = 10;

#[derive(Debug, Clone, Copy)]
pub struct Read {
  pub reg: u8,
  pub len: u8
}

#[derive(Debug, Clone, Copy)]
pub struct Write {
  pub reg: u8,
  pub len: u8,
  pub data: [u8; TX_BUFFER_SIZE]
}

#[derive(Debug)]
pub enum Message {
  Ignore,
  StartRead(Read),
  StartWrite(Write),
  TxEvent,
  RxEvent,
  Error,
  FinishTransaction,
  CancelTransaction,
  ReadConfirmation,
  WriteConfirmation
}

#[derive(Debug)]
pub enum Action {
  DoNothing,
  Reset,
  Reject,
  StartRead(Read),
  StartWrite(Write),
  ContinueRead,
  ContinueWrite,
}