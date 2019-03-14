use crate::isolate::Behavior;
use crate::isolate::Op;
use crate::libdeno::deno_buf;
use crate::libdeno::deno_mod;
use std::collections::HashMap;

pub struct TestBehavior {
  pub dispatch_count: usize,
  pub resolve_count: usize,
  pub mod_map: HashMap<String, deno_mod>,
}

impl TestBehavior {
  pub fn new() -> Self {
    Self {
      dispatch_count: 0,
      resolve_count: 0,
      mod_map: HashMap::new(),
    }
  }

  pub fn register(&mut self, name: &str, id: deno_mod) {
    self.mod_map.insert(name.to_string(), id);
  }
}

impl Behavior for TestBehavior {
  fn startup_snapshot(&mut self) -> Option<deno_buf> {
    None
  }

  fn dispatch(
    &mut self,
    control: &[u8],
    _zero_copy_buf: deno_buf,
  ) -> (bool, Box<Op>) {
    assert_eq!(control.len(), 1);
    assert_eq!(control[0], 42);
    self.dispatch_count += 1;
    let buf = vec![43u8].into_boxed_slice();
    (false, Box::new(futures::future::ok(buf)))
  }

  fn resolve(&mut self, specifier: &str, _referrer: deno_mod) -> deno_mod {
    self.resolve_count += 1;
    match self.mod_map.get(specifier) {
      Some(id) => *id,
      None => 0,
    }
  }
}
