// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::Result;
use std::collections::HashMap;
use std::path::Path;
pub use test_util::{parse_wrk_output, WrkOutput as HttpBenchmarkResult};
use crate::http::{get_port, run};

pub fn benchmark() -> Result<HashMap<String, HttpBenchmarkResult>> {
  let deno_exe = test_util::deno_exe_path();
  let deno_exe = deno_exe.to_str().unwrap();

  let mut res = HashMap::new();
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let runtimes_dir = manifest_dir.join("bench").join("runtimes_dir");
    
  // node <path> <port>
  {
    let port = get_port();
    let path = runtimes_dir.join("ssr/react-hello-world-node.js").to_str().unwrap().to_string();
    res.insert(
      "node".to_string(),
      run(
        &["node", &path, &port.to_string()],
        port,
        None,
        None,
        None,
      )?,
    );
  } 

  // bun <path> <port>
  {
    let port = get_port();
    let path = runtimes_dir.join("ssr/react-hello-world-bun.js").to_str().unwrap().to_string();

    // Bun does not support Windows.
    #[cfg(target_arch = "x86_64")]
    #[cfg(not(target_vendor = "apple"))]
    let bun_exe = test_util::prebuilt_tool_path("bun");
    #[cfg(target_vendor = "apple")]
    #[cfg(target_arch = "x86_64")]
    let bun_exe = test_util::prebuilt_tool_path("bun-x64");
    #[cfg(target_vendor = "apple")]
    #[cfg(target_arch = "aarch64")]
    let bun_exe = test_util::prebuilt_tool_path("bun-aarch64");

    res.insert(
      "bun".to_string(),
      run(
        &[bun_exe.to_str().unwrap(), &path, &port.to_string()],
        port,
        None,
        None,
        None,
      )?,
    );
  }
  
  // deno run -A --unstable <path> <addr>
  {
    let port = get_port();
    let path = runtimes_dir.join("ssr/react-hello-world-deno.js").to_str().unwrap().to_string();
    res.insert(
      "deno".to_string(),
      run(
        &[
          deno_exe,
          "run",
          "--allow-all",
          "--unstable",
          &path,
          &port.to_string(),
        ],
        port,
        None,
        None,
        None,
      )?,
    );
  }

  Ok(res)
}
