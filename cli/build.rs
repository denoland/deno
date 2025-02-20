// Copyright 2018-2025 the Deno authors. MIT license.

use std::env;
use std::io::Write;
use std::path::Path;

use deno_runtime::*;

fn compress_decls(out_dir: &Path) {
  let decls = [
    "lib.deno_webgpu.d.ts",
    "lib.deno.ns.d.ts",
    "lib.deno.unstable.d.ts",
    "lib.deno.window.d.ts",
    "lib.deno.worker.d.ts",
    "lib.deno.shared_globals.d.ts",
    "lib.deno.ns.d.ts",
    "lib.deno.unstable.d.ts",
    "lib.deno_console.d.ts",
    "lib.deno_url.d.ts",
    "lib.deno_web.d.ts",
    "lib.deno_fetch.d.ts",
    "lib.deno_websocket.d.ts",
    "lib.deno_webstorage.d.ts",
    "lib.deno_canvas.d.ts",
    "lib.deno_crypto.d.ts",
    "lib.deno_cache.d.ts",
    "lib.deno_net.d.ts",
    "lib.deno_broadcast_channel.d.ts",
    "lib.decorators.d.ts",
    "lib.decorators.legacy.d.ts",
    "lib.dom.asynciterable.d.ts",
    "lib.dom.d.ts",
    "lib.dom.extras.d.ts",
    "lib.dom.iterable.d.ts",
    "lib.es2015.collection.d.ts",
    "lib.es2015.core.d.ts",
    "lib.es2015.d.ts",
    "lib.es2015.generator.d.ts",
    "lib.es2015.iterable.d.ts",
    "lib.es2015.promise.d.ts",
    "lib.es2015.proxy.d.ts",
    "lib.es2015.reflect.d.ts",
    "lib.es2015.symbol.d.ts",
    "lib.es2015.symbol.wellknown.d.ts",
    "lib.es2016.array.include.d.ts",
    "lib.es2016.d.ts",
    "lib.es2016.full.d.ts",
    "lib.es2016.intl.d.ts",
    "lib.es2017.arraybuffer.d.ts",
    "lib.es2017.d.ts",
    "lib.es2017.date.d.ts",
    "lib.es2017.full.d.ts",
    "lib.es2017.intl.d.ts",
    "lib.es2017.object.d.ts",
    "lib.es2017.sharedmemory.d.ts",
    "lib.es2017.string.d.ts",
    "lib.es2017.typedarrays.d.ts",
    "lib.es2018.asyncgenerator.d.ts",
    "lib.es2018.asynciterable.d.ts",
    "lib.es2018.d.ts",
    "lib.es2018.full.d.ts",
    "lib.es2018.intl.d.ts",
    "lib.es2018.promise.d.ts",
    "lib.es2018.regexp.d.ts",
    "lib.es2019.array.d.ts",
    "lib.es2019.d.ts",
    "lib.es2019.full.d.ts",
    "lib.es2019.intl.d.ts",
    "lib.es2019.object.d.ts",
    "lib.es2019.string.d.ts",
    "lib.es2019.symbol.d.ts",
    "lib.es2020.bigint.d.ts",
    "lib.es2020.d.ts",
    "lib.es2020.date.d.ts",
    "lib.es2020.full.d.ts",
    "lib.es2020.intl.d.ts",
    "lib.es2020.number.d.ts",
    "lib.es2020.promise.d.ts",
    "lib.es2020.sharedmemory.d.ts",
    "lib.es2020.string.d.ts",
    "lib.es2020.symbol.wellknown.d.ts",
    "lib.es2021.d.ts",
    "lib.es2021.full.d.ts",
    "lib.es2021.intl.d.ts",
    "lib.es2021.promise.d.ts",
    "lib.es2021.string.d.ts",
    "lib.es2021.weakref.d.ts",
    "lib.es2022.array.d.ts",
    "lib.es2022.d.ts",
    "lib.es2022.error.d.ts",
    "lib.es2022.full.d.ts",
    "lib.es2022.intl.d.ts",
    "lib.es2022.object.d.ts",
    "lib.es2022.regexp.d.ts",
    "lib.es2022.string.d.ts",
    "lib.es2023.array.d.ts",
    "lib.es2023.collection.d.ts",
    "lib.es2023.d.ts",
    "lib.es2023.full.d.ts",
    "lib.es2023.intl.d.ts",
    "lib.es2024.arraybuffer.d.ts",
    "lib.es2024.collection.d.ts",
    "lib.es2024.d.ts",
    "lib.es2024.full.d.ts",
    "lib.es2024.object.d.ts",
    "lib.es2024.promise.d.ts",
    "lib.es2024.regexp.d.ts",
    "lib.es2024.sharedmemory.d.ts",
    "lib.es2024.string.d.ts",
    "lib.es5.d.ts",
    "lib.es6.d.ts",
    "lib.esnext.array.d.ts",
    "lib.esnext.collection.d.ts",
    "lib.esnext.d.ts",
    "lib.esnext.decorators.d.ts",
    "lib.esnext.disposable.d.ts",
    "lib.esnext.full.d.ts",
    "lib.esnext.intl.d.ts",
    "lib.esnext.iterator.d.ts",
    "lib.scripthost.d.ts",
    "lib.webworker.asynciterable.d.ts",
    "lib.webworker.d.ts",
    "lib.webworker.importscripts.d.ts",
    "lib.webworker.iterable.d.ts",
  ];
  for decl in decls {
    let file = format!("./tsc/dts/{decl}");
    compress_source(out_dir, &file);
  }
}

fn compress_source(out_dir: &Path, file: &str) {
  let path = Path::new(file)
    .canonicalize()
    .unwrap_or_else(|_| panic!("expected file \"{file}\" to exist"));
  let contents = std::fs::read(&path).unwrap();

  println!("cargo:rerun-if-changed={}", path.display());

  let compressed = zstd::bulk::compress(&contents, 19).unwrap();
  let mut out = out_dir.join(file.trim_start_matches("../"));
  let mut ext = out
    .extension()
    .map(|s| s.to_string_lossy())
    .unwrap_or_default()
    .into_owned();
  ext.push_str(".zstd");
  out.set_extension(ext);
  std::fs::create_dir_all(out.parent().unwrap()).unwrap();
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(out)
    .unwrap();
  file
    .write_all(&(contents.len() as u32).to_le_bytes())
    .unwrap();

  file.write_all(&compressed).unwrap();
}

fn compress_sources(out_dir: &Path) {
  compress_decls(out_dir);

  let ext_sources = [
    "./tsc/99_main_compiler.js",
    "./tsc/97_ts_host.js",
    "./tsc/98_lsp.js",
    "./tsc/00_typescript.js",
  ];
  for ext_source in ext_sources {
    compress_source(out_dir, ext_source);
  }
}

fn main() {
  // Skip building from docs.rs.
  if env::var_os("DOCS_RS").is_some() {
    return;
  }

  deno_napi::print_linker_flags("deno");
  deno_webgpu::print_linker_flags("deno");

  // Host snapshots won't work when cross compiling.
  let target = env::var("TARGET").unwrap();
  let host = env::var("HOST").unwrap();
  let skip_cross_check =
    env::var("DENO_SKIP_CROSS_BUILD_CHECK").map_or(false, |v| v == "1");
  if !skip_cross_check && target != host {
    panic!("Cross compiling with snapshot is not supported.");
  }

  // To debug snapshot issues uncomment:
  // op_fetch_asset::trace_serializer();

  if !cfg!(debug_assertions) && std::env::var("CARGO_FEATURE_HMR").is_err() {
    let out_dir =
      std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    compress_sources(&out_dir);
  }

  if let Ok(c) = env::var("DENO_CANARY") {
    println!("cargo:rustc-env=DENO_CANARY={c}");
  }
  println!("cargo:rerun-if-env-changed=DENO_CANARY");

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());

  #[cfg(target_os = "windows")]
  {
    let mut res = winres::WindowsResource::new();
    res.set_icon("deno.ico");
    res.set_language(winapi::um::winnt::MAKELANGID(
      winapi::um::winnt::LANG_ENGLISH,
      winapi::um::winnt::SUBLANG_ENGLISH_US,
    ));
    res.compile().unwrap();
  }
}
