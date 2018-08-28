// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use errors::DenoError;
use errors::DenoResult;
use fs as deno_fs;
use net;
use ring;
use std;
use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::result::Result;
#[cfg(test)]
use tempfile::TempDir;
use url;
use url::Url;

pub struct DenoDir {
  // Example: /Users/rld/.deno/
  pub root: PathBuf,
  // In the Go code this was called SrcDir.
  // This is where we cache http resources. Example:
  // /Users/rld/.deno/deps/github.com/ry/blah.js
  pub gen: PathBuf,
  // In the Go code this was called CacheDir.
  // This is where we cache compilation outputs. Example:
  // /Users/rld/.deno/gen/f39a473452321cacd7c346a870efb0e3e1264b43.js
  pub deps: PathBuf,
  // If remote resources should be reloaded.
  reload: bool,
}

impl DenoDir {
  // Must be called before using any function from this module.
  // https://github.com/denoland/deno/blob/golang/deno_dir.go#L99-L111
  pub fn new(
    reload: bool,
    custom_root: Option<&Path>,
  ) -> std::io::Result<DenoDir> {
    // Only setup once.
    let home_dir = std::env::home_dir().expect("Could not get home directory.");
    let default = home_dir.join(".deno");

    let root: PathBuf = match custom_root {
      None => default,
      Some(path) => path.to_path_buf(),
    };
    let gen = root.as_path().join("gen");
    let deps = root.as_path().join("deps");

    let deno_dir = DenoDir {
      root,
      gen,
      deps,
      reload,
    };
    deno_fs::mkdir(deno_dir.gen.as_ref())?;
    deno_fs::mkdir(deno_dir.deps.as_ref())?;

    debug!("root {}", deno_dir.root.display());
    debug!("gen {}", deno_dir.gen.display());
    debug!("deps {}", deno_dir.deps.display());

    Ok(deno_dir)
  }

  // https://github.com/denoland/deno/blob/golang/deno_dir.go#L32-L35
  pub fn cache_path(
    self: &DenoDir,
    filename: &str,
    source_code: &str,
  ) -> PathBuf {
    let cache_key = source_code_hash(filename, source_code);
    self.gen.join(cache_key + ".js")
  }

  fn load_cache(
    self: &DenoDir,
    filename: &str,
    source_code: &str,
  ) -> std::io::Result<String> {
    let path = self.cache_path(filename, source_code);
    debug!("load_cache {}", path.display());
    fs::read_to_string(&path)
  }

  pub fn code_cache(
    self: &DenoDir,
    filename: &str,
    source_code: &str,
    output_code: &str,
  ) -> std::io::Result<()> {
    let cache_path = self.cache_path(filename, source_code);
    // TODO(ry) This is a race condition w.r.t to exists() -- probably should
    // create the file in exclusive mode. A worry is what might happen is there
    // are two processes and one reads the cache file while the other is in the
    // midst of writing it.
    if cache_path.exists() {
      Ok(())
    } else {
      fs::write(cache_path, output_code.as_bytes())
    }
  }

  // Prototype https://github.com/denoland/deno/blob/golang/deno_dir.go#L37-L73
  fn fetch_remote_source(
    self: &DenoDir,
    module_name: &str,
    filename: &str,
  ) -> DenoResult<String> {
    let p = Path::new(filename);

    let src = if self.reload || !p.exists() {
      println!("Downloading {}", module_name);
      let source = net::fetch_sync_string(module_name)?;
      match p.parent() {
        Some(ref parent) => fs::create_dir_all(parent),
        None => Ok(()),
      }?;
      deno_fs::write_file_sync(&p, source.as_bytes())?;
      source
    } else {
      let source = fs::read_to_string(&p)?;
      source
    };
    Ok(src)
  }

  // Prototype: https://github.com/denoland/deno/blob/golang/os.go#L122-L138
  fn get_source_code(
    self: &DenoDir,
    module_name: &str,
    filename: &str,
  ) -> DenoResult<String> {
    if is_remote(module_name) {
      self.fetch_remote_source(module_name, filename)
    } else if module_name.starts_with(ASSET_PREFIX) {
      panic!("Asset resolution should be done in JS, not Rust.");
    } else {
      assert!(
        module_name == filename,
        "if a module isn't remote, it should have the same filename"
      );
      let src = fs::read_to_string(Path::new(filename))?;
      Ok(src)
    }
  }

  pub fn code_fetch(
    self: &DenoDir,
    module_specifier: &str,
    containing_file: &str,
  ) -> Result<CodeFetchOutput, DenoError> {
    let (module_name, filename) =
      self.resolve_module(module_specifier, containing_file)?;

    debug!(
            "code_fetch. module_name = {} module_specifier = {} containing_file = {} filename = {}",
            module_name, module_specifier, containing_file, filename
        );

    let out = self
      .get_source_code(module_name.as_str(), filename.as_str())
      .and_then(|source_code| {
        Ok(CodeFetchOutput {
          module_name,
          filename,
          source_code,
          maybe_output_code: None,
        })
      })?;

    let result =
      self.load_cache(out.filename.as_str(), out.source_code.as_str());
    match result {
      Err(err) => {
        if err.kind() == std::io::ErrorKind::NotFound {
          Ok(out)
        } else {
          Err(err.into())
        }
      }
      Ok(output_code) => Ok(CodeFetchOutput {
        module_name: out.module_name,
        filename: out.filename,
        source_code: out.source_code,
        maybe_output_code: Some(output_code),
      }),
    }
  }

  // Prototype: https://github.com/denoland/deno/blob/golang/os.go#L56-L68
  #[allow(dead_code)]
  fn src_file_to_url<P: AsRef<Path>>(self: &DenoDir, filename: P) -> String {
    let filename = filename.as_ref().to_path_buf();
    if filename.starts_with(&self.deps) {
      let rest = filename.strip_prefix(&self.deps).unwrap();
      "http://".to_string() + rest.to_str().unwrap()
    } else {
      String::from(filename.to_str().unwrap())
    }
  }

  // Prototype: https://github.com/denoland/deno/blob/golang/os.go#L70-L98
  // Returns (module name, local filename)
  fn resolve_module(
    self: &DenoDir,
    module_specifier: &str,
    containing_file: &str,
  ) -> Result<(String, String), url::ParseError> {
    let module_name;
    let filename;

    debug!(
      "resolve_module before module_specifier {} containing_file {}",
      module_specifier, containing_file
    );

    //let module_specifier = src_file_to_url(module_specifier);
    //let containing_file = src_file_to_url(containing_file);
    //let base_url = Url::parse(&containing_file)?;

    let j: Url =
      if containing_file == "." || Path::new(module_specifier).is_absolute() {
        if module_specifier.starts_with("http://") {
          Url::parse(module_specifier)?
        } else {
          Url::from_file_path(module_specifier).unwrap()
        }
      } else if containing_file.ends_with("/") {
        let r = Url::from_directory_path(&containing_file);
        // TODO(ry) Properly handle error.
        if r.is_err() {
          error!("Url::from_directory_path error {}", containing_file);
        }
        let base = r.unwrap();
        base.join(module_specifier)?
      } else {
        let r = Url::from_file_path(&containing_file);
        // TODO(ry) Properly handle error.
        if r.is_err() {
          error!("Url::from_file_path error {}", containing_file);
        }
        let base = r.unwrap();
        base.join(module_specifier)?
      };

    match j.scheme() {
      "file" => {
        let mut p = deno_fs::normalize_path(j.to_file_path().unwrap().as_ref());
        module_name = p.clone();
        filename = p;
      }
      _ => {
        module_name = module_specifier.to_string();
        filename = deno_fs::normalize_path(
          get_cache_filename(self.deps.as_path(), j).as_ref(),
        )
      }
    }

    debug!("module_name: {}, filename: {}", module_name, filename);
    Ok((module_name, filename))
  }
}

fn get_cache_filename(basedir: &Path, url: Url) -> PathBuf {
  let mut out = basedir.to_path_buf();
  out.push(url.host_str().unwrap());
  for path_seg in url.path_segments().unwrap() {
    out.push(path_seg);
  }
  out
}

#[test]
fn test_get_cache_filename() {
  let url = Url::parse("http://example.com:1234/path/to/file.ts").unwrap();
  let basedir = Path::new("/cache/dir/");
  let cache_file = get_cache_filename(&basedir, url);
  assert_eq!(
    cache_file,
    Path::new("/cache/dir/example.com/path/to/file.ts")
  );
}

#[derive(Debug)]
pub struct CodeFetchOutput {
  pub module_name: String,
  pub filename: String,
  pub source_code: String,
  pub maybe_output_code: Option<String>,
}

#[cfg(test)]
pub fn test_setup() -> (TempDir, DenoDir) {
  let temp_dir = TempDir::new().expect("tempdir fail");
  let deno_dir =
    DenoDir::new(false, Some(temp_dir.path())).expect("setup fail");
  (temp_dir, deno_dir)
}

#[test]
fn test_cache_path() {
  let (temp_dir, deno_dir) = test_setup();
  assert_eq!(
    temp_dir
      .path()
      .join("gen/a3e29aece8d35a19bf9da2bb1c086af71fb36ed5.js"),
    deno_dir.cache_path("hello.ts", "1+2")
  );
}

#[test]
fn test_code_cache() {
  let (_temp_dir, deno_dir) = test_setup();

  let filename = "hello.js";
  let source_code = "1+2";
  let output_code = "1+2 // output code";
  let cache_path = deno_dir.cache_path(filename, source_code);
  assert!(
    cache_path.ends_with("gen/e8e3ee6bee4aef2ec63f6ec3db7fc5fdfae910ae.js")
  );

  let r = deno_dir.code_cache(filename, source_code, output_code);
  r.expect("code_cache error");
  assert!(cache_path.exists());
  assert_eq!(output_code, fs::read_to_string(&cache_path).unwrap());
}

// https://github.com/denoland/deno/blob/golang/deno_dir.go#L25-L30
fn source_code_hash(filename: &str, source_code: &str) -> String {
  let mut ctx = ring::digest::Context::new(&ring::digest::SHA1);
  ctx.update(filename.as_bytes());
  ctx.update(source_code.as_bytes());
  let digest = ctx.finish();
  let mut out = String::new();
  // TODO There must be a better way to do this...
  for byte in digest.as_ref() {
    write!(&mut out, "{:02x}", byte).unwrap();
  }
  out
}

#[test]
fn test_source_code_hash() {
  assert_eq!(
    "a3e29aece8d35a19bf9da2bb1c086af71fb36ed5",
    source_code_hash("hello.ts", "1+2")
  );
  // Different source_code should result in different hash.
  assert_eq!(
    "914352911fc9c85170908ede3df1128d690dda41",
    source_code_hash("hello.ts", "1")
  );
  // Different filename should result in different hash.
  assert_eq!(
    "2e396bc66101ecc642db27507048376d972b1b70",
    source_code_hash("hi.ts", "1+2")
  );
}

// The `add_root` macro prepends "C:" to a string if on windows; on posix
// systems it returns the input string untouched. This is necessary because
// `Url::from_file_path()` fails if the input path isn't an absolute path.
#[cfg(test)]
macro_rules! add_root {
  ($path:expr) => {
    if cfg!(target_os = "windows") {
      concat!("C:", $path)
    } else {
      $path
    }
  };
}

#[test]
fn test_code_fetch() {
  let (_temp_dir, deno_dir) = test_setup();

  let cwd = std::env::current_dir().unwrap();
  let cwd_string = String::from(cwd.to_str().unwrap()) + "/";

  // Test failure case.
  let module_specifier = "hello.ts";
  let containing_file = add_root!("/baddir/badfile.ts");
  let r = deno_dir.code_fetch(module_specifier, containing_file);
  assert!(r.is_err());

  // Assuming cwd is the deno repo root.
  let module_specifier = "./js/main.ts";
  let containing_file = cwd_string.as_str();
  let r = deno_dir.code_fetch(module_specifier, containing_file);
  assert!(r.is_ok());
  //let code_fetch_output = r.unwrap();
  //println!("code_fetch_output {:?}", code_fetch_output);
}

#[test]
fn test_src_file_to_url() {
  let (_temp_dir, deno_dir) = test_setup();
  assert_eq!("hello", deno_dir.src_file_to_url("hello"));
  assert_eq!("/hello", deno_dir.src_file_to_url("/hello"));
  let x = String::from(deno_dir.deps.join("hello/world.txt").to_str().unwrap());
  assert_eq!("http://hello/world.txt", deno_dir.src_file_to_url(x));
}

// https://github.com/denoland/deno/blob/golang/os_test.go#L16-L87
#[test]
fn test_resolve_module() {
  let (_temp_dir, deno_dir) = test_setup();

  let d = deno_fs::normalize_path(
    deno_dir
      .deps
      .join("localhost/testdata/subdir/print_hello.ts")
      .as_ref(),
  );

  let test_cases = [
    (
      "./subdir/print_hello.ts",
      add_root!(
        "/Users/rld/go/src/github.com/denoland/deno/testdata/006_url_imports.ts"
      ),
      add_root!(
        "/Users/rld/go/src/github.com/denoland/deno/testdata/subdir/print_hello.ts"
      ),
      add_root!(
        "/Users/rld/go/src/github.com/denoland/deno/testdata/subdir/print_hello.ts"
      ),
    ),
    (
      "testdata/001_hello.js",
      add_root!("/Users/rld/go/src/github.com/denoland/deno/"),
      add_root!("/Users/rld/go/src/github.com/denoland/deno/testdata/001_hello.js"),
      add_root!("/Users/rld/go/src/github.com/denoland/deno/testdata/001_hello.js"),
    ),
    (
      add_root!("/Users/rld/src/deno/hello.js"),
      ".",
      add_root!("/Users/rld/src/deno/hello.js"),
      add_root!("/Users/rld/src/deno/hello.js"),
    ),
    (
      add_root!("/this/module/got/imported.js"),
      add_root!("/that/module/did/it.js"),
      add_root!("/this/module/got/imported.js"),
      add_root!("/this/module/got/imported.js"),
    ),
    (
        "http://localhost:4545/testdata/subdir/print_hello.ts",
        add_root!("/Users/rld/go/src/github.com/denoland/deno/testdata/006_url_imports.ts"),
        "http://localhost:4545/testdata/subdir/print_hello.ts",
        d.as_ref(),
    ),
    /*
        (
            path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"),
            ".",
            "http://unpkg.com/liltest@0.0.5/index.ts",
            path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"),
        ),
        (
            "./util",
            path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"),
            "http://unpkg.com/liltest@0.0.5/util",
            path.Join(SrcDir, "unpkg.com/liltest@0.0.5/util"),
        ),
        */
  ];
  for &test in test_cases.iter() {
    let module_specifier = String::from(test.0);
    let containing_file = String::from(test.1);
    let (module_name, filename) = deno_dir
      .resolve_module(&module_specifier, &containing_file)
      .unwrap();
    assert_eq!(module_name, test.2);
    assert_eq!(filename, test.3);
  }
}

const ASSET_PREFIX: &str = "/$asset$/";

fn is_remote(module_name: &str) -> bool {
  module_name.starts_with("http")
}
