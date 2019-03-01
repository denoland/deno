use crate::isolate::IsolateInitScript;

#[cfg(not(feature = "use-snapshots"))]
pub fn deno_init_script() -> Option<IsolateInitScript> {
  if cfg!(feature = "check-only") {
    Some(IsolateInitScript {
      filename: "".to_string(),
      source: "".to_string(),
    })
  } else {
    Some(IsolateInitScript {
      filename: "gen/bundle/main.js".to_string(),
      source: std::str::from_utf8(include_bytes!(concat!(
        env!("GN_OUT_DIR"),
        "/gen/bundle/main.js"
      ))).unwrap()
      .to_string(),
    })
  }
}

#[cfg(feature = "use-snapshots")]
pub fn deno_init_script() -> Option<IsolateInitScript> {
  None
}

#[cfg(not(feature = "use-snapshots"))]
pub fn compiler_init_script() -> Option<IsolateInitScript> {
  if cfg!(feature = "check-only") {
    Some(IsolateInitScript {
      filename: "".to_string(),
      source: "".to_string(),
    })
  } else {
    Some(IsolateInitScript {
      filename: "gen/bundle/compiler.js".to_string(),
      source: std::str::from_utf8(include_bytes!(concat!(
        env!("GN_OUT_DIR"),
        "/gen/bundle/compiler.js"
      ))).unwrap()
      .to_string(),
    })
  }
}

#[cfg(feature = "use-snapshots")]
pub fn compiler_init_script() -> Option<IsolateInitScript> {
  None
}
