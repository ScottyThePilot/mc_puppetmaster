use std::sync::atomic::{AtomicBool, Ordering};

pub struct AtomicFlag(AtomicBool);

impl AtomicFlag {
  #[inline]
  pub fn new() -> Self {
    AtomicFlag(AtomicBool::new(false))
  }

  pub fn set(&self) {
    self.0.store(true, Ordering::Relaxed);
  }

  pub fn get(&self) -> bool {
    self.0.load(Ordering::Relaxed)
  }
}
