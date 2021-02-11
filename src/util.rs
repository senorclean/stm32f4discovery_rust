use crate::constants;

pub const fn convert_us_to_cycles(us: u32) -> u32 {
  us * (constants::CPU_FREQ / 1_000_000)
}

pub fn is_debugger_connected() -> bool {
  let addr = 0xE000EDF0usize;
  let r = addr as *const u32;
  if unsafe{*r & 1} == 1 {
    true
  } else {
    false
  }
}
