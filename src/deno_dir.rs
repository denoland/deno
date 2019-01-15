// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::compiler::CodeFetchOutput;
use crate::errors;
use crate::errors::DenoError;
use crate::errors::DenoResult;
use crate::errors::ErrorKind;
use crate::fs as deno_fs;
use crate::http_util;
use crate::js_errors::SourceMapGetter;
use crate::msg;

use dirs;
use ring;
use std;
use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::result::Result;
use url;
use url::Url;

/// Gets corresponding MediaType given extension
fn extmap(ext: &str) -> msg::MediaType {
  match ext {
    "ts" => msg::MediaType::TypeScript,
    "js" => msg::MediaType::JavaScript,
    "json" => msg::MediaType::Json,
    _ => msg::MediaType::Unknown,
  }
}

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
  // This splits to http and https deps
  pub deps_http: PathBuf,
  pub deps_https: PathBuf,
  // If remote resources should be reloaded.
  reload: bool,
}

impl DenoDir {
  // Must be called before using any function from this module.
  // https://github.com/denoland/deno/blob/golang/deno_dir.go#L99-L111
  pub fn new(
    reload: bool,
    custom_root: Option<PathBuf>,
  ) -> std::io::Result<Self> {
    // Only setup once.
    let home_dir = dirs::home_dir().expect("Could not get home directory.");
    let default = home_dir.join(".deno");

    let root: PathBuf = custom_root.unwrap_or(default);
    let gen = root.as_path().join("gen");
    let deps = root.as_path().join("deps");
    let deps_http = deps.join("http");
    let deps_https = deps.join("https");

    let deno_dir = Self {
      root,
      gen,
      deps,
      deps_http,
      deps_https,
      reload,
    };
    deno_fs::mkdir(deno_dir.gen.as_ref(), 0o755)?;
    deno_fs::mkdir(deno_dir.deps.as_ref(), 0o755)?;
    deno_fs::mkdir(deno_dir.deps_http.as_ref(), 0o755)?;
    deno_fs::mkdir(deno_dir.deps_https.as_ref(), 0o755)?;

    debug!("root {}", deno_dir.root.display());
    debug!("gen {}", deno_dir.gen.display());
    debug!("deps {}", deno_dir.deps.display());
    debug!("deps_http {}", deno_dir.deps_http.display());
    debug!("deps_https {}", deno_dir.deps_https.display());

    Ok(deno_dir)
  }

  // https://github.com/denoland/deno/blob/golang/deno_dir.go#L32-L35
  pub fn cache_path(
    self: &Self,
    filename: &str,
    source_code: &str,
  ) -> (PathBuf, PathBuf) {
    let cache_key = source_code_hash(filename, source_code);
    (
      self.gen.join(cache_key.to_string() + ".js"),
      self.gen.join(cache_key.to_string() + ".js.map"),
    )
  }

  fn load_cache(
    self: &Self,
    filename: &str,
    source_code: &str,
  ) -> Result<(String, String), std::io::Error> {
    let (output_code, source_map) = self.cache_path(filename, source_code);
    debug!(
      "load_cache code: {} map: {}",
      output_code.display(),
      source_map.display()
    );
    let read_output_code = fs::read_to_string(&output_code)?;
    let read_source_map = fs::read_to_string(&source_map)?;
    Ok((read_output_code, read_source_map))
  }

  pub fn code_cache(
    self: &Self,
    filename: &str,
    source_code: &str,
    output_code: &str,
    source_map: &str,
  ) -> std::io::Result<()> {
    let (cache_path, source_map_path) = self.cache_path(filename, source_code);
    // TODO(ry) This is a race condition w.r.t to exists() -- probably should
    // create the file in exclusive mode. A worry is what might happen is there
    // are two processes and one reads the cache file while the other is in the
    // midst of writing it.
    if cache_path.exists() && source_map_path.exists() {
      Ok(())
    } else {
      fs::write(cache_path, output_code.as_bytes())?;
      fs::write(source_map_path, source_map.as_bytes())?;
      Ok(())
    }
  }

  // Prototype https://github.com/denoland/deno/blob/golang/deno_dir.go#L37-L73
  /// Fetch remote source code.
  fn fetch_remote_source(
    self: &Self,
    module_name: &str,
    filename: &str,
  ) -> DenoResult<Option<CodeFetchOutput>> {
    let p = Path::new(&filename);
    // We write a special ".mime" file into the `.deno/deps` directory along side the
    // cached file, containing just the media type.
    let media_type_filename = [&filename, ".mime"].concat();
    let mt = Path::new(&media_type_filename);
    eprint!("Downloading {}...", &module_name); // no newline
    let maybe_source = http_util::fetch_sync_string(&module_name);
    if let Ok((source, content_type)) = maybe_source {
      eprintln!(""); // next line
      match p.parent() {
        Some(ref parent) => fs::create_dir_all(parent),
        None => Ok(()),
      }?;
      deno_fs::write_file(&p, &source, 0o666)?;
      // Remove possibly existing stale .mime file
      // may not exist. DON'T unwrap
      let _ = std::fs::remove_file(&media_type_filename);
      // Create .mime file only when content type different from extension
      let resolved_content_type = map_content_type(&p, Some(&content_type));
      let ext = p
        .extension()
        .map(|x| x.to_str().unwrap_or(""))
        .unwrap_or("");
      let media_type = extmap(&ext);
      if media_type == msg::MediaType::Unknown
        || media_type != resolved_content_type
      {
        deno_fs::write_file(&mt, content_type.as_bytes(), 0o666)?
      }
      return Ok(Some(CodeFetchOutput {
        module_name: module_name.to_string(),
        filename: filename.to_string(),
        media_type: map_content_type(&p, Some(&content_type)),
        source_code: source,
        maybe_output_code: None,
        maybe_source_map: None,
      }));
    } else {
      eprintln!(" NOT FOUND");
    }
    Ok(None)
  }

  /// Fetch local or cached source code.
  fn fetch_local_source(
    self: &Self,
    module_name: &str,
    filename: &str,
  ) -> DenoResult<Option<CodeFetchOutput>> {
    let p = Path::new(&filename);
    let media_type_filename = [&filename, ".mime"].concat();
    let mt = Path::new(&media_type_filename);
    let source_code = match fs::read(p) {
      Err(e) => {
        if e.kind() == std::io::ErrorKind::NotFound {
          return Ok(None);
        } else {
          return Err(e.into());
        }
      }
      Ok(c) => String::from_utf8(c).unwrap(),
    };
    // .mime file might not exists
    // this is okay for local source: maybe_content_type_str will be None
    let maybe_content_type_string = fs::read_to_string(&mt).ok();
    // Option<String> -> Option<&str>
    let maybe_content_type_str =
      maybe_content_type_string.as_ref().map(String::as_str);
    Ok(Some(CodeFetchOutput {
      module_name: module_name.to_string(),
      filename: filename.to_string(),
      media_type: map_content_type(&p, maybe_content_type_str),
      source_code,
      maybe_output_code: None,
      maybe_source_map: None,
    }))
  }

  // Prototype: https://github.com/denoland/deno/blob/golang/os.go#L122-L138
  fn get_source_code(
    self: &Self,
    module_name: &str,
    filename: &str,
  ) -> DenoResult<CodeFetchOutput> {
    let is_module_remote = is_remote(module_name);
    // We try fetch local. Two cases:
    // 1. This is a remote module, but no reload provided
    // 2. This is a local module
    if !is_module_remote || !self.reload {
      debug!(
        "fetch local or reload {} is_module_remote {}",
        module_name, is_module_remote
      );
      match self.fetch_local_source(&module_name, &filename)? {
        Some(output) => {
          debug!("found local source ");
          return Ok(output);
        }
        None => {
          debug!("fetch_local_source returned None");
        }
      }
    }

    // If not remote file, stop here!
    if !is_module_remote {
      debug!("not remote file stop here");
      return Err(DenoError::from(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("cannot find local file '{}'", filename),
      )));
    }

    debug!("is remote but didn't find module");

    // not cached/local, try remote
    let maybe_remote_source =
      self.fetch_remote_source(&module_name, &filename)?;
    if let Some(output) = maybe_remote_source {
      return Ok(output);
    }
    Err(DenoError::from(std::io::Error::new(
      std::io::ErrorKind::NotFound,
      format!("cannot find remote file '{}'", filename),
    )))
  }

  pub fn code_fetch(
    self: &Self,
    specifier: &str,
    referrer: &str,
  ) -> Result<CodeFetchOutput, errors::DenoError> {
    debug!("code_fetch. specifier {} referrer {}", specifier, referrer);

    let (module_name, filename) = self.resolve_module(specifier, referrer)?;

    let result = self.get_source_code(module_name.as_str(), filename.as_str());
    let mut out = match result {
      Ok(out) => out,
      Err(err) => {
        if err.kind() == ErrorKind::NotFound {
          // For NotFound, change the message to something better.
          return Err(errors::new(
            ErrorKind::NotFound,
            format!(
              "Cannot resolve module \"{}\" from \"{}\"",
              specifier, referrer
            ),
          ));
        } else {
          return Err(err);
        }
      }
    };

    out.source_code = filter_shebang(out.source_code);

    if out.media_type != msg::MediaType::TypeScript {
      return Ok(out);
    }

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
      Ok((output_code, source_map)) => Ok(CodeFetchOutput {
        module_name: out.module_name,
        filename: out.filename,
        media_type: out.media_type,
        source_code: out.source_code,
        maybe_output_code: Some(output_code),
        maybe_source_map: Some(source_map),
      }),
    }
  }

  // Prototype: https://github.com/denoland/deno/blob/golang/os.go#L56-L68
  fn src_file_to_url(self: &Self, filename: &str) -> String {
    let filename_path = Path::new(filename);
    if filename_path.starts_with(&self.deps) {
      let (rest, prefix) = if filename_path.starts_with(&self.deps_https) {
        let rest = filename_path.strip_prefix(&self.deps_https).unwrap();
        let prefix = "https://".to_string();
        (rest, prefix)
      } else if filename_path.starts_with(&self.deps_http) {
        let rest = filename_path.strip_prefix(&self.deps_http).unwrap();
        let prefix = "http://".to_string();
        (rest, prefix)
      } else {
        // TODO(kevinkassimo): change this to support other protocols than http
        unimplemented!()
      };
      // Windows doesn't support ":" in filenames, so we represent port using a
      // special string.
      // TODO(ry) This current implementation will break on a URL that has
      // the default port but contains "_PORT" in the path.
      let rest = rest.to_str().unwrap().replacen("_PORT", ":", 1);
      prefix + &rest
    } else {
      String::from(filename)
    }
  }

  // Prototype: https://github.com/denoland/deno/blob/golang/os.go#L70-L98
  // Returns (module name, local filename)
  fn resolve_module(
    self: &Self,
    specifier: &str,
    referrer: &str,
  ) -> Result<(String, String), url::ParseError> {
    let module_name;
    let filename;

    let specifier = self.src_file_to_url(specifier);
    let mut referrer = self.src_file_to_url(referrer);

    debug!(
      "resolve_module specifier {} referrer {}",
      specifier, referrer
    );

    if referrer.starts_with('.') {
      let cwd = std::env::current_dir().unwrap();
      let referrer_path = cwd.join(referrer);
      referrer = referrer_path.to_str().unwrap().to_string() + "/";
    }

    let j: Url = if is_remote(&specifier) || Path::new(&specifier).is_absolute()
    {
      parse_local_or_remote(&specifier)?
    } else if referrer.ends_with('/') {
      let r = Url::from_directory_path(&referrer);
      // TODO(ry) Properly handle error.
      if r.is_err() {
        error!("Url::from_directory_path error {}", referrer);
      }
      let base = r.unwrap();
      base.join(specifier.as_ref())?
    } else {
      let base = parse_local_or_remote(&referrer)?;
      base.join(specifier.as_ref())?
    };

    match j.scheme() {
      "file" => {
        let p = deno_fs::normalize_path(j.to_file_path().unwrap().as_ref());
        module_name = p.clone();
        filename = p;
      }
      "https" => {
        module_name = j.to_string();
        filename = deno_fs::normalize_path(
          get_cache_filename(self.deps_https.as_path(), &j).as_ref(),
        )
      }
      "http" => {
        module_name = j.to_string();
        filename = deno_fs::normalize_path(
          get_cache_filename(self.deps_http.as_path(), &j).as_ref(),
        )
      }
      // TODO(kevinkassimo): change this to support other protocols than http
      _ => unimplemented!(),
    }

    debug!("module_name: {}, filename: {}", module_name, filename);
    Ok((module_name, filename))
  }
}

impl SourceMapGetter for DenoDir {
  fn get_source_map(&self, script_name: &str) -> Option<String> {
    match self.code_fetch(script_name, ".") {
      Err(_e) => None,
      Ok(out) => match out.maybe_source_map {
        None => None,
        Some(source_map) => Some(source_map),
      },
    }
  }
}

fn get_cache_filename(basedir: &Path, url: &Url) -> PathBuf {
  let host = url.host_str().unwrap();
  let host_port = match url.port() {
    // Windows doesn't support ":" in filenames, so we represent port using a
    // special string.
    Some(port) => format!("{}_PORT{}", host, port),
    None => host.to_string(),
  };

  let mut out = basedir.to_path_buf();
  out.push(host_port);
  for path_seg in url.path_segments().unwrap() {
    out.push(path_seg);
  }
  out
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

fn is_remote(module_name: &str) -> bool {
  module_name.starts_with("http://") || module_name.starts_with("https://")
}

fn parse_local_or_remote(p: &str) -> Result<url::Url, url::ParseError> {
  if is_remote(p) {
    Url::parse(p)
  } else {
    Url::from_file_path(p).map_err(|_err| url::ParseError::IdnaError)
  }
}

fn map_file_extension(path: &Path) -> msg::MediaType {
  match path.extension() {
    None => msg::MediaType::Unknown,
    Some(os_str) => match os_str.to_str() {
      Some("ts") => msg::MediaType::TypeScript,
      Some("js") => msg::MediaType::JavaScript,
      Some("json") => msg::MediaType::Json,
      _ => msg::MediaType::Unknown,
    },
  }
}

// convert a ContentType string into a enumerated MediaType
fn map_content_type(path: &Path, content_type: Option<&str>) -> msg::MediaType {
  match content_type {
    Some(content_type) => {
      // sometimes there is additional data after the media type in
      // Content-Type so we have to do a bit of manipulation so we are only
      // dealing with the actual media type
      let ct_vector: Vec<&str> = content_type.split(';').collect();
      let ct: &str = ct_vector.first().unwrap();
      match ct.to_lowercase().as_ref() {
        "application/typescript"
        | "text/typescript"
        | "video/vnd.dlna.mpeg-tts"
        | "video/mp2t"
        | "application/x-typescript" => msg::MediaType::TypeScript,
        "application/javascript"
        | "text/javascript"
        | "application/ecmascript"
        | "text/ecmascript"
        | "application/x-javascript" => msg::MediaType::JavaScript,
        "application/json" | "text/json" => msg::MediaType::Json,
        "text/plain" => map_file_extension(path),
        _ => {
          debug!("unknown content type: {}", content_type);
          msg::MediaType::Unknown
        }
      }
    }
    None => map_file_extension(path),
  }
}

fn filter_shebang(code: String) -> String {
  if !code.starts_with("#!") {
    return code;
  }
  if let Some(i) = code.find('\n') {
    let (_, rest) = code.split_at(i);
    String::from(rest)
  } else {
    String::from("")
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tokio_util;
  use tempfile::TempDir;

  fn test_setup() -> (TempDir, DenoDir) {
    let temp_dir = TempDir::new().expect("tempdir fail");
    let deno_dir = DenoDir::new(false, Some(temp_dir.path().to_path_buf()))
      .expect("setup fail");
    (temp_dir, deno_dir)
  }

  // The `add_root` macro prepends "C:" to a string if on windows; on posix
  // systems it returns the input string untouched. This is necessary because
  // `Url::from_file_path()` fails if the input path isn't an absolute path.
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
  fn test_get_cache_filename() {
    let url = Url::parse("http://example.com:1234/path/to/file.ts").unwrap();
    let basedir = Path::new("/cache/dir/");
    let cache_file = get_cache_filename(&basedir, &url);
    assert_eq!(
      cache_file,
      Path::new("/cache/dir/example.com_PORT1234/path/to/file.ts")
    );
  }

  #[test]
  fn test_cache_path() {
    let (temp_dir, deno_dir) = test_setup();
    assert_eq!(
      (
        temp_dir
          .path()
          .join("gen/a3e29aece8d35a19bf9da2bb1c086af71fb36ed5.js"),
        temp_dir
          .path()
          .join("gen/a3e29aece8d35a19bf9da2bb1c086af71fb36ed5.js.map")
      ),
      deno_dir.cache_path("hello.ts", "1+2")
    );
  }

  #[test]
  fn test_code_cache() {
    let (_temp_dir, deno_dir) = test_setup();

    let filename = "hello.js";
    let source_code = "1+2";
    let output_code = "1+2 // output code";
    let source_map = "{}";
    let (cache_path, source_map_path) =
      deno_dir.cache_path(filename, source_code);
    assert!(
      cache_path.ends_with("gen/e8e3ee6bee4aef2ec63f6ec3db7fc5fdfae910ae.js")
    );
    assert!(
      source_map_path
        .ends_with("gen/e8e3ee6bee4aef2ec63f6ec3db7fc5fdfae910ae.js.map")
    );

    let r = deno_dir.code_cache(filename, source_code, output_code, source_map);
    r.expect("code_cache error");
    assert!(cache_path.exists());
    assert_eq!(output_code, fs::read_to_string(&cache_path).unwrap());
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

  #[test]
  fn test_get_source_code_1() {
    let (temp_dir, deno_dir) = test_setup();
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let module_name = "http://localhost:4545/tests/subdir/mod2.ts";
      let filename = deno_fs::normalize_path(
        deno_dir
          .deps_http
          .join("localhost_PORT4545/tests/subdir/mod2.ts")
          .as_ref(),
      );
      let mime_file_name = format!("{}.mime", &filename);

      let result = deno_dir.get_source_code(module_name, &filename);
      println!("module_name {} filename {}", module_name, filename);
      assert!(result.is_ok());
      let r = result.unwrap();
      assert_eq!(
        &(r.source_code),
        "export { printHello } from \"./print_hello.ts\";\n"
      );
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // Should not create .mime file due to matching ext
      assert!(fs::read_to_string(&mime_file_name).is_err());

      // Modify .mime
      let _ = fs::write(&mime_file_name, "text/javascript");
      let result2 = deno_dir.get_source_code(module_name, &filename);
      assert!(result2.is_ok());
      let r2 = result2.unwrap();
      assert_eq!(
        &(r2.source_code),
        "export { printHello } from \"./print_hello.ts\";\n"
      );
      // If get_source_code does not call remote, this should be JavaScript
      // as we modified before! (we do not overwrite .mime due to no http fetch)
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        fs::read_to_string(&mime_file_name).unwrap(),
        "text/javascript"
      );

      // Force self.reload
      let deno_dir = DenoDir::new(true, Some(temp_dir.path().to_path_buf()))
        .expect("setup fail");
      let result3 = deno_dir.get_source_code(module_name, &filename);
      assert!(result3.is_ok());
      let r3 = result3.unwrap();
      let expected3 = "export { printHello } from \"./print_hello.ts\";\n";
      assert_eq!(r3.source_code, expected3);
      // Now the old .mime file should have gone! Resolved back to TypeScript
      assert_eq!(&(r3.media_type), &msg::MediaType::TypeScript);
      assert!(fs::read_to_string(&mime_file_name).is_err());
    });
  }

  #[test]
  fn test_get_source_code_2() {
    let (temp_dir, deno_dir) = test_setup();
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let module_name = "http://localhost:4545/tests/subdir/mismatch_ext.ts";
      let filename = deno_fs::normalize_path(
        deno_dir
          .deps_http
          .join("localhost_PORT4545/tests/subdir/mismatch_ext.ts")
          .as_ref(),
      );
      let mime_file_name = format!("{}.mime", &filename);

      let result = deno_dir.get_source_code(module_name, &filename);
      println!("module_name {} filename {}", module_name, filename);
      assert!(result.is_ok());
      let r = result.unwrap();
      let expected = "export const loaded = true;\n";
      assert_eq!(r.source_code, expected);
      // Mismatch ext with content type, create .mime
      assert_eq!(&(r.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        fs::read_to_string(&mime_file_name).unwrap(),
        "text/javascript"
      );

      // Modify .mime
      let _ = fs::write(&mime_file_name, "text/typescript");
      let result2 = deno_dir.get_source_code(module_name, &filename);
      assert!(result2.is_ok());
      let r2 = result2.unwrap();
      let expected2 = "export const loaded = true;\n";
      assert_eq!(r2.source_code, expected2);
      // If get_source_code does not call remote, this should be TypeScript
      // as we modified before! (we do not overwrite .mime due to no http fetch)
      assert_eq!(&(r2.media_type), &msg::MediaType::TypeScript);
      assert_eq!(
        fs::read_to_string(&mime_file_name).unwrap(),
        "text/typescript"
      );

      // Force self.reload
      let deno_dir = DenoDir::new(true, Some(temp_dir.path().to_path_buf()))
        .expect("setup fail");
      let result3 = deno_dir.get_source_code(module_name, &filename);
      assert!(result3.is_ok());
      let r3 = result3.unwrap();
      let expected3 = "export const loaded = true;\n";
      assert_eq!(r3.source_code, expected3);
      // Now the old .mime file should be overwritten back to JavaScript!
      // (due to http fetch)
      assert_eq!(&(r3.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        fs::read_to_string(&mime_file_name).unwrap(),
        "text/javascript"
      );
    });
  }

  #[test]
  fn test_fetch_source_1() {
    use crate::tokio_util;
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let (_temp_dir, deno_dir) = test_setup();
      let module_name =
        "http://localhost:4545/tests/subdir/mt_video_mp2t.t3.ts";
      let filename = deno_fs::normalize_path(
        deno_dir
          .deps_http
          .join("localhost_PORT4545/tests/subdir/mt_video_mp2t.t3.ts")
          .as_ref(),
      );
      let mime_file_name = format!("{}.mime", &filename);

      let result = deno_dir.fetch_remote_source(module_name, &filename);
      assert!(result.is_ok());
      let r = result.unwrap().unwrap();
      assert_eq!(&(r.source_code), "export const loaded = true;\n");
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // matching ext, no .mime file created
      assert!(fs::read_to_string(&mime_file_name).is_err());

      // Modify .mime, make sure read from local
      let _ = fs::write(&mime_file_name, "text/javascript");
      let result2 = deno_dir.fetch_local_source(module_name, &filename);
      assert!(result2.is_ok());
      let r2 = result2.unwrap().unwrap();
      assert_eq!(&(r2.source_code), "export const loaded = true;\n");
      // Not MediaType::TypeScript due to .mime modification
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
    });
  }

  #[test]
  fn test_fetch_source_2() {
    use crate::tokio_util;
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let (_temp_dir, deno_dir) = test_setup();
      let module_name = "http://localhost:4545/tests/subdir/no_ext";
      let filename = deno_fs::normalize_path(
        deno_dir
          .deps_http
          .join("localhost_PORT4545/tests/subdir/no_ext")
          .as_ref(),
      );
      let mime_file_name = format!("{}.mime", &filename);
      let result = deno_dir.fetch_remote_source(module_name, &filename);
      assert!(result.is_ok());
      let r = result.unwrap().unwrap();
      assert_eq!(&(r.source_code), "export const loaded = true;\n");
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // no ext, should create .mime file
      assert_eq!(
        fs::read_to_string(&mime_file_name).unwrap(),
        "text/typescript"
      );

      let module_name_2 = "http://localhost:4545/tests/subdir/mismatch_ext.ts";
      let filename_2 = deno_fs::normalize_path(
        deno_dir
          .deps_http
          .join("localhost_PORT4545/tests/subdir/mismatch_ext.ts")
          .as_ref(),
      );
      let mime_file_name_2 = format!("{}.mime", &filename_2);
      let result_2 = deno_dir.fetch_remote_source(module_name_2, &filename_2);
      assert!(result_2.is_ok());
      let r2 = result_2.unwrap().unwrap();
      assert_eq!(&(r2.source_code), "export const loaded = true;\n");
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
      // mismatch ext, should create .mime file
      assert_eq!(
        fs::read_to_string(&mime_file_name_2).unwrap(),
        "text/javascript"
      );

      // test unknown extension
      let module_name_3 = "http://localhost:4545/tests/subdir/unknown_ext.deno";
      let filename_3 = deno_fs::normalize_path(
        deno_dir
          .deps_http
          .join("localhost_PORT4545/tests/subdir/unknown_ext.deno")
          .as_ref(),
      );
      let mime_file_name_3 = format!("{}.mime", &filename_3);
      let result_3 = deno_dir.fetch_remote_source(module_name_3, &filename_3);
      assert!(result_3.is_ok());
      let r3 = result_3.unwrap().unwrap();
      assert_eq!(&(r3.source_code), "export const loaded = true;\n");
      assert_eq!(&(r3.media_type), &msg::MediaType::TypeScript);
      // unknown ext, should create .mime file
      assert_eq!(
        fs::read_to_string(&mime_file_name_3).unwrap(),
        "text/typescript"
      );
    });
  }

  #[test]
  fn test_fetch_source_3() {
    // only local, no http_util::fetch_sync_string called
    let (_temp_dir, deno_dir) = test_setup();
    let cwd = std::env::current_dir().unwrap();
    let cwd_string = cwd.to_str().unwrap();
    let module_name = "http://example.com/mt_text_typescript.t1.ts"; // not used
    let filename =
      format!("{}/tests/subdir/mt_text_typescript.t1.ts", &cwd_string);

    let result = deno_dir.fetch_local_source(module_name, &filename);
    assert!(result.is_ok());
    let r = result.unwrap().unwrap();
    assert_eq!(&(r.source_code), "export const loaded = true;\n");
    assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
  }

  #[test]
  fn test_code_fetch() {
    let (_temp_dir, deno_dir) = test_setup();

    let cwd = std::env::current_dir().unwrap();
    let cwd_string = String::from(cwd.to_str().unwrap()) + "/";

    // Test failure case.
    let specifier = "hello.ts";
    let referrer = add_root!("/baddir/badfile.ts");
    let r = deno_dir.code_fetch(specifier, referrer);
    assert!(r.is_err());

    // Assuming cwd is the deno repo root.
    let specifier = "./js/main.ts";
    let referrer = cwd_string.as_str();
    let r = deno_dir.code_fetch(specifier, referrer);
    assert!(r.is_ok());
    //let code_fetch_output = r.unwrap();
    //println!("code_fetch_output {:?}", code_fetch_output);
  }

  #[test]
  fn test_src_file_to_url_1() {
    let (_temp_dir, deno_dir) = test_setup();
    assert_eq!("hello", deno_dir.src_file_to_url("hello"));
    assert_eq!("/hello", deno_dir.src_file_to_url("/hello"));
    let x = deno_dir.deps_http.join("hello/world.txt");
    assert_eq!(
      "http://hello/world.txt",
      deno_dir.src_file_to_url(x.to_str().unwrap())
    );
  }

  #[test]
  fn test_src_file_to_url_2() {
    let (_temp_dir, deno_dir) = test_setup();
    assert_eq!("hello", deno_dir.src_file_to_url("hello"));
    assert_eq!("/hello", deno_dir.src_file_to_url("/hello"));
    let x = deno_dir.deps_https.join("hello/world.txt");
    assert_eq!(
      "https://hello/world.txt",
      deno_dir.src_file_to_url(x.to_str().unwrap())
    );
  }

  #[test]
  fn test_src_file_to_url_3() {
    let (_temp_dir, deno_dir) = test_setup();
    let x = deno_dir.deps_http.join("localhost_PORT4545/world.txt");
    assert_eq!(
      "http://localhost:4545/world.txt",
      deno_dir.src_file_to_url(x.to_str().unwrap())
    );
  }

  #[test]
  fn test_src_file_to_url_4() {
    let (_temp_dir, deno_dir) = test_setup();
    let x = deno_dir.deps_https.join("localhost_PORT4545/world.txt");
    assert_eq!(
      "https://localhost:4545/world.txt",
      deno_dir.src_file_to_url(x.to_str().unwrap())
    );
  }

  // https://github.com/denoland/deno/blob/golang/os_test.go#L16-L87
  #[test]
  fn test_resolve_module_1() {
    let (_temp_dir, deno_dir) = test_setup();

    let test_cases = [
      (
        "./subdir/print_hello.ts",
        add_root!("/Users/rld/go/src/github.com/denoland/deno/testdata/006_url_imports.ts"),
        add_root!("/Users/rld/go/src/github.com/denoland/deno/testdata/subdir/print_hello.ts"),
        add_root!("/Users/rld/go/src/github.com/denoland/deno/testdata/subdir/print_hello.ts"),
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
    ];
    for &test in test_cases.iter() {
      let specifier = String::from(test.0);
      let referrer = String::from(test.1);
      let (module_name, filename) =
        deno_dir.resolve_module(&specifier, &referrer).unwrap();
      assert_eq!(module_name, test.2);
      assert_eq!(filename, test.3);
    }
  }

  #[test]
  fn test_resolve_module_2() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier = "http://localhost:4545/testdata/subdir/print_hello.ts";
    let referrer = add_root!("/deno/testdata/006_url_imports.ts");

    let expected_module_name =
      "http://localhost:4545/testdata/subdir/print_hello.ts";
    let expected_filename = deno_fs::normalize_path(
      deno_dir
        .deps_http
        .join("localhost_PORT4545/testdata/subdir/print_hello.ts")
        .as_ref(),
    );

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, referrer).unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);
  }

  #[test]
  fn test_resolve_module_3() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier_ =
      deno_dir.deps_http.join("unpkg.com/liltest@0.0.5/index.ts");
    let specifier = specifier_.to_str().unwrap();
    let referrer = ".";

    let expected_module_name = "http://unpkg.com/liltest@0.0.5/index.ts";
    let expected_filename = deno_fs::normalize_path(
      deno_dir
        .deps_http
        .join("unpkg.com/liltest@0.0.5/index.ts")
        .as_ref(),
    );

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, referrer).unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);
  }

  #[test]
  fn test_resolve_module_4() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier = "./util";
    let referrer_ = deno_dir.deps_http.join("unpkg.com/liltest@0.0.5/index.ts");
    let referrer = referrer_.to_str().unwrap();

    // http containing files -> load relative import with http
    let expected_module_name = "http://unpkg.com/liltest@0.0.5/util";
    let expected_filename = deno_fs::normalize_path(
      deno_dir
        .deps_http
        .join("unpkg.com/liltest@0.0.5/util")
        .as_ref(),
    );

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, referrer).unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);
  }

  #[test]
  fn test_resolve_module_5() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier = "./util";
    let referrer_ =
      deno_dir.deps_https.join("unpkg.com/liltest@0.0.5/index.ts");
    let referrer = referrer_.to_str().unwrap();

    // https containing files -> load relative import with https
    let expected_module_name = "https://unpkg.com/liltest@0.0.5/util";
    let expected_filename = deno_fs::normalize_path(
      deno_dir
        .deps_https
        .join("unpkg.com/liltest@0.0.5/util")
        .as_ref(),
    );

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, referrer).unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);
  }

  #[test]
  fn test_resolve_module_6() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier = "http://localhost:4545/tests/subdir/mod2.ts";
    let referrer = add_root!("/deno/tests/006_url_imports.ts");
    let expected_module_name = "http://localhost:4545/tests/subdir/mod2.ts";
    let expected_filename = deno_fs::normalize_path(
      deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/mod2.ts")
        .as_ref(),
    );

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, referrer).unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);
  }

  #[test]
  fn test_resolve_module_7() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier = "http_test.ts";
    let referrer = add_root!("/Users/rld/src/deno_net/");
    let expected_module_name =
      add_root!("/Users/rld/src/deno_net/http_test.ts");
    let expected_filename = add_root!("/Users/rld/src/deno_net/http_test.ts");

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, referrer).unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);
  }

  #[test]
  fn test_resolve_module_referrer_dot() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier = "tests/001_hello.js";

    let cwd = std::env::current_dir().unwrap();
    let expected_path = cwd.join(specifier);
    let expected_module_name = deno_fs::normalize_path(&expected_path);
    let expected_filename = expected_module_name.clone();

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, ".").unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, "./").unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);
  }

  #[test]
  fn test_resolve_module_referrer_dotdot() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier = "tests/001_hello.js";

    let cwd = std::env::current_dir().unwrap();
    let expected_path = cwd.join("..").join(specifier);
    let expected_module_name = deno_fs::normalize_path(&expected_path);
    let expected_filename = expected_module_name.clone();

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, "..").unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);

    let (module_name, filename) =
      deno_dir.resolve_module(specifier, "../").unwrap();
    assert_eq!(module_name, expected_module_name);
    assert_eq!(filename, expected_filename);
  }

  #[test]
  fn test_map_file_extension() {
    assert_eq!(
      map_file_extension(Path::new("foo/bar.ts")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.d.ts")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.js")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.json")),
      msg::MediaType::Json
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.txt")),
      msg::MediaType::Unknown
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar")),
      msg::MediaType::Unknown
    );
  }

  #[test]
  fn test_map_content_type() {
    // Extension only
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), None),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.d.ts"), None),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.js"), None),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.json"), None),
      msg::MediaType::Json
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.txt"), None),
      msg::MediaType::Unknown
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), None),
      msg::MediaType::Unknown
    );

    // Media Type
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/typescript")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/typescript")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("video/vnd.dlna.mpeg-tts")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("video/mp2t")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/x-typescript")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/javascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/javascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/ecmascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/ecmascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/x-javascript")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("application/json")),
      msg::MediaType::Json
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/json")),
      msg::MediaType::Json
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), Some("text/plain")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), Some("foo/bar")),
      msg::MediaType::Unknown
    );
  }

  #[test]
  fn test_filter_shebang() {
    assert_eq!(filter_shebang("".to_string()), "");
    assert_eq!(filter_shebang("#".to_string()), "#");
    assert_eq!(filter_shebang("#!".to_string()), "");
    assert_eq!(filter_shebang("#!\n\n".to_string()), "\n\n");
    let code = "#!/usr/bin/env deno\nconsole.log('hello');\n".to_string();
    assert_eq!(filter_shebang(code), "\nconsole.log('hello');\n");
  }
}
