use stm32f4xx_hal::{
  prelude::*,
  pwm::PwmChannels,
  pwm::C2,
  stm32::TIM4,
  // delay::Delay,
};

use rtic::cyccnt::{Instant, U32Ext};
#[derive(Debug)]
pub enum Messages {
  TurnOff,
  TurnOn,
  Toggle
}

pub fn heartbeat(led: &mut PwmChannels<TIM4, C2>, increment: &mut bool) {
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