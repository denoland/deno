use crate::fs as deno_fs;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use url::Url;

#[derive(Clone)]
pub struct DiskCache {
  pub location: PathBuf,
}

impl DiskCache {
  pub fn new(location: &Path) -> Self {
    // TODO: ensure that 'location' is a directory
    Self {
      location: location.to_owned(),
    }
  }

  // TODO(bartlomieju) this method is not working properly for Windows paths,
  // Example: file:///C:/deno/js/unit_test_runner.ts
  // would produce: C:deno\\js\\unit_test_runner.ts
  // it should produce: file\deno\js\unit_test_runner.ts
  pub fn get_cache_filename(self: &Self, url: &Url) -> PathBuf {
    let mut out = PathBuf::new();

    let scheme = url.scheme();
    out.push(scheme);
    match scheme {
      "http" | "https" => {
        let host = url.host_str().unwrap();
        let host_port = match url.port() {
          // Windows doesn't support ":" in filenames, so we represent port using a
          // special string.
          Some(port) => format!("{}_PORT{}", host, port),
          None => host.to_string(),
        };
        out.push(host_port);
      }
      _ => {}
    };

    for path_seg in url.path_segments().unwrap() {
      out.push(path_seg);
    }
    out
  }

  pub fn get_cache_filename_with_extension(
    self: &Self,
    url: &Url,
    extension: &str,
  ) -> PathBuf {
    let base = self.get_cache_filename(url);

    match base.extension() {
      None => base.with_extension(extension),
      Some(ext) => {
        let original_extension = OsStr::to_str(ext).unwrap();
        let final_extension = format!("{}.{}", original_extension, extension);
        base.with_extension(final_extension)
      }
    }
  }

  pub fn get(self: &Self, filename: &Path) -> std::io::Result<Vec<u8>> {
    let path = self.location.join(filename);
    fs::read(&path)
  }

  pub fn set(self: &Self, filename: &Path, data: &[u8]) -> std::io::Result<()> {
    let path = self.location.join(filename);
    match path.parent() {
      Some(ref parent) => fs::create_dir_all(parent),
      None => Ok(()),
    }?;
    deno_fs::write_file(&path, data, 0o666)
  }

  pub fn remove(self: &Self, filename: &Path) -> std::io::Result<()> {
    let path = self.location.join(filename);
    fs::remove_file(path)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_cache_filename() {
    let cache = DiskCache::new(&PathBuf::from("foo"));

    let test_cases = [
      (
        "http://deno.land/std/http/file_server.ts",
        "http/deno.land/std/http/file_server.ts",
      ),
      (
        "http://localhost:8000/std/http/file_server.ts",
        "http/localhost_PORT8000/std/http/file_server.ts",
      ),
      (
        "https://deno.land/std/http/file_server.ts",
        "https/deno.land/std/http/file_server.ts",
      ),
      (
        "file:///std/http/file_server.ts",
        "file/std/http/file_server.ts",
      ),
    ];

    for test_case in &test_cases {
      assert_eq!(
        cache.get_cache_filename(&Url::parse(test_case.0).unwrap()),
        PathBuf::from(test_case.1)
      )
    }
  }

  #[test]
  fn test_get_cache_filename_with_extension() {
    let cache = DiskCache::new(&PathBuf::from("foo"));

    let test_cases = [
      (
        "http://deno.land/std/http/file_server.ts",
        "js",
        "http/deno.land/std/http/file_server.ts.js",
      ),
      (
        "file:///std/http/file_server",
        "js",
        "file/std/http/file_server.js",
      ),
      (
        "http://deno.land/std/http/file_server.ts",
        "js.map",
        "http/deno.land/std/http/file_server.ts.js.map",
      ),
    ];

    for test_case in &test_cases {
      assert_eq!(
        cache.get_cache_filename_with_extension(
          &Url::parse(test_case.0).unwrap(),
          test_case.1
        ),
        PathBuf::from(test_case.2)
      )
    }
  }
}
