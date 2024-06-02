use lazy_static::lazy_static;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

lazy_static! {
  pub static ref RUNNING_FROM_BINARY: AtomicBool = AtomicBool::new(false);
}

pub fn set_running_from_binary(value: bool) {
  RUNNING_FROM_BINARY.store(value, Ordering::SeqCst);
}

pub fn is_running_from_binary() -> bool {
  RUNNING_FROM_BINARY.load(Ordering::SeqCst)
}
