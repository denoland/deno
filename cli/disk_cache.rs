use crate::fs as deno_fs;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::path::Prefix;
use std::str;
use url::Url;

#[derive(Clone)]
pub struct DiskCache {
  pub location: PathBuf,
}

fn with_io_context<T: AsRef<str>>(
  e: &std::io::Error,
  context: T,
) -> std::io::Error {
  std::io::Error::new(e.kind(), format!("{} (for '{}')", e, context.as_ref()))
}

impl DiskCache {
  /// `location` must be an absolute path.
  pub fn new(location: &Path) -> Self {
    assert!(location.is_absolute());
    Self {
      location: location.to_owned(),
    }
  }

  /// Ensures the location of the cache.
  pub fn ensure_dir_exists(&self, path: &Path) -> io::Result<()> {
    if path.is_dir() {
      return Ok(());
    }
    fs::create_dir_all(&path).map_err(|e| {
      io::Error::new(e.kind(), format!(
        "Could not create TypeScript compiler cache location: {:?}\nCheck the permission of the directory.",
        path
      ))
    })
  }

  pub fn get_cache_filename(&self, url: &Url) -> PathBuf {
    let mut out = PathBuf::new();

    let scheme = url.scheme();
    out.push(scheme);

    match scheme {
      "http" | "https" | "wasm" => {
        let host = url.host_str().unwrap();
        let host_port = match url.port() {
          // Windows doesn't support ":" in filenames, so we represent port using a
          // special string.
          Some(port) => format!("{}_PORT{}", host, port),
          None => host.to_string(),
        };
        out.push(host_port);

        for path_seg in url.path_segments().unwrap() {
          out.push(path_seg);
        }
      }
      "file" => {
        let path = url.to_file_path().unwrap();
        let mut path_components = path.components();

        if cfg!(target_os = "windows") {
          if let Some(Component::Prefix(prefix_component)) =
            path_components.next()
          {
            // Windows doesn't support ":" in filenames, so we need to extract disk prefix
            // Example: file:///C:/deno/js/unit_test_runner.ts
            // it should produce: file\c\deno\js\unit_test_runner.ts
            match prefix_component.kind() {
              Prefix::Disk(disk_byte) | Prefix::VerbatimDisk(disk_byte) => {
                let disk = (disk_byte as char).to_string();
                out.push(disk);
              }
              _ => unreachable!(),
            }
          }
        }

        // Must be relative, so strip forward slash
        let mut remaining_components = path_components.as_path();
        if let Ok(stripped) = remaining_components.strip_prefix("/") {
          remaining_components = stripped;
        };

        out = out.join(remaining_components);
      }
      scheme => {
        unimplemented!(
          "Don't know how to create cache name for scheme: {}",
          scheme
        );
      }
    };

    out
  }

  pub fn get_cache_filename_with_extension(
    &self,
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

  pub fn get(&self, filename: &Path) -> std::io::Result<Vec<u8>> {
    let path = self.location.join(filename);
    fs::read(&path)
  }

  pub fn set(&self, filename: &Path, data: &[u8]) -> std::io::Result<()> {
    let path = self.location.join(filename);
    match path.parent() {
      Some(ref parent) => self.ensure_dir_exists(parent),
      None => Ok(()),
    }?;
    deno_fs::write_file(&path, data, 0o666)
      .map_err(|e| with_io_context(&e, format!("{:#?}", &path)))
  }

  pub fn remove(&self, filename: &Path) -> std::io::Result<()> {
    let path = self.location.join(filename);
    fs::remove_file(path)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn test_create_cache_if_dir_exits() {
    let cache_location = TempDir::new().unwrap();
    let mut cache_path = cache_location.path().to_owned();
    cache_path.push("foo");
    let cache = DiskCache::new(&cache_path);
    cache
      .ensure_dir_exists(&cache.location)
      .expect("Testing expect:");
    assert!(cache_path.is_dir());
  }

  #[test]
  fn test_create_cache_if_dir_not_exits() {
    let temp_dir = TempDir::new().unwrap();
    let mut cache_location = temp_dir.path().to_owned();
    assert!(fs::remove_dir(&cache_location).is_ok());
    cache_location.push("foo");
    assert_eq!(cache_location.is_dir(), false);
    let cache = DiskCache::new(&cache_location);
    cache
      .ensure_dir_exists(&cache.location)
      .expect("Testing expect:");
    assert_eq!(cache_location.is_dir(), true);
  }

  #[test]
  fn test_get_cache_filename() {
    let cache_location = if cfg!(target_os = "windows") {
      PathBuf::from(r"C:\deno_dir\")
    } else {
      PathBuf::from("/deno_dir/")
    };

    let cache = DiskCache::new(&cache_location);

    let mut test_cases = vec![
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
      ("wasm://wasm/d1c677ea", "wasm/wasm/d1c677ea"),
    ];

    if cfg!(target_os = "windows") {
      test_cases.push(("file:///D:/a/1/s/format.ts", "file/D/a/1/s/format.ts"));
    } else {
      test_cases.push((
        "file:///std/http/file_server.ts",
        "file/std/http/file_server.ts",
      ));
    }

    for test_case in &test_cases {
      let cache_filename =
        cache.get_cache_filename(&Url::parse(test_case.0).unwrap());
      assert_eq!(cache_filename, PathBuf::from(test_case.1));
    }
  }

  #[test]
  fn test_get_cache_filename_with_extension() {
    let p = if cfg!(target_os = "windows") {
      "C:\\foo"
    } else {
      "/foo"
    };
    let cache = DiskCache::new(&PathBuf::from(p));

    let mut test_cases = vec![
      (
        "http://deno.land/std/http/file_server.ts",
        "js",
        "http/deno.land/std/http/file_server.ts.js",
      ),
      (
        "http://deno.land/std/http/file_server.ts",
        "js.map",
        "http/deno.land/std/http/file_server.ts.js.map",
      ),
    ];

    if cfg!(target_os = "windows") {
      test_cases.push((
        "file:///D:/std/http/file_server",
        "js",
        "file/D/std/http/file_server.js",
      ));
    } else {
      test_cases.push((
        "file:///std/http/file_server",
        "js",
        "file/std/http/file_server.js",
      ));
    }

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
