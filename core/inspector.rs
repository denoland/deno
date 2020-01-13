#![allow(unused)]

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct InspectorHandle {
  pub tx: Arc<Mutex<Sender<String>>>,
  pub rx: Arc<Mutex<Receiver<String>>>,
}

impl InspectorHandle {
  pub fn new(tx: Sender<String>, rx: Receiver<String>) -> Self {
    InspectorHandle {
      tx: Arc::new(Mutex::new(tx)),
      rx: Arc::new(Mutex::new(rx)),
    }
  }
}
