// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno_core::deno_buf;

pub struct IsolateInitScript {
  pub source: String,
  pub filename: String,
}

pub struct IsolateInit {
  pub snapshot: Option<deno_buf>,
  pub init_script: Option<IsolateInitScript>,
}

pub fn deno_isolate_init() -> IsolateInit {
  if cfg!(feature = "no-snapshot-init") {
    debug!("Deno isolate init without snapshots.");
    #[cfg(not(feature = "check-only"))]
    let source_bytes =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/bundle/main.js"));
    #[cfg(feature = "check-only")]
    let source_bytes = vec![];

    IsolateInit {
      snapshot: None,
      init_script: Some(IsolateInitScript {
        filename: "gen/bundle/main.js".to_string(),
        source: std::str::from_utf8(source_bytes).unwrap().to_string(),
      }),
    }
  } else {
    debug!("Deno isolate init with snapshots.");
    #[cfg(not(feature = "check-only"))]
    let data =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_deno.bin"));
    #[cfg(feature = "check-only")]
    let data = vec![];

    unsafe {
      IsolateInit {
        snapshot: Some(deno_buf::from_raw_parts(data.as_ptr(), data.len())),
        init_script: None,
      }
    }
  }
}

pub fn compiler_isolate_init() -> IsolateInit {
  if cfg!(feature = "no-snapshot-init") {
    debug!("Deno isolate init without snapshots.");
    #[cfg(not(feature = "check-only"))]
    let source_bytes =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/bundle/compiler.js"));
    #[cfg(feature = "check-only")]
    let source_bytes = vec![];

    IsolateInit {
      snapshot: None,
      init_script: Some(IsolateInitScript {
        filename: "gen/bundle/compiler.js".to_string(),
        source: std::str::from_utf8(source_bytes).unwrap().to_string(),
      }),
    }
  } else {
    debug!("Deno isolate init with snapshots.");
    #[cfg(not(feature = "check-only"))]
    let data =
      include_bytes!(concat!(env!("GN_OUT_DIR"), "/gen/snapshot_compiler.bin"));
    #[cfg(feature = "check-only")]
    let data = vec![];

    unsafe {
      IsolateInit {
        snapshot: Some(deno_buf::from_raw_parts(data.as_ptr(), data.len())),
        init_script: None,
      }
    }
  }
}
