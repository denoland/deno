// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error::too_many_redirects;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::deno_error::GetErrorKind;
use crate::disk_cache::DiskCache;
use crate::http_util;
use crate::msg;
use crate::progress::Progress;
use crate::tokio_util;
use deno::ErrBox;
use deno::ModuleSpecifier;
use futures::future::Either;
use futures::Future;
use http;
use serde_json;
use std;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::result::Result;
use std::str;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use url;
use url::Url;

/// Structure representing local or remote file.
///
/// In case of remote file `url` might be different than originally requested URL, if so
/// `redirect_source_url` will contain original URL and `url` will be equal to final location.
#[derive(Debug, Clone)]
pub struct SourceFile {
  pub url: Url,
  pub filename: PathBuf,
  pub media_type: msg::MediaType,
  pub source_code: Vec<u8>,
}

pub type SourceFileFuture =
  dyn Future<Item = SourceFile, Error = ErrBox> + Send;

/// Simple struct implementing in-process caching to prevent multiple
/// fs reads/net fetches for same file.
#[derive(Clone, Default)]
pub struct SourceFileCache(Arc<Mutex<HashMap<String, SourceFile>>>);

impl SourceFileCache {
  pub fn set(&self, key: String, source_file: SourceFile) {
    let mut c = self.0.lock().unwrap();
    c.insert(key, source_file);
  }

  pub fn get(&self, key: String) -> Option<SourceFile> {
    let c = self.0.lock().unwrap();
    match c.get(&key) {
      Some(source_file) => Some(source_file.clone()),
      None => None,
    }
  }
}

const SUPPORTED_URL_SCHEMES: [&str; 3] = ["http", "https", "file"];

/// `DenoDir` serves as coordinator for multiple `DiskCache`s containing them
/// in single directory that can be controlled with `$DENO_DIR` env variable.
#[derive(Clone)]
pub struct SourceFileFetcher {
  deps_cache: DiskCache,
  progress: Progress,
  source_file_cache: SourceFileCache,
  use_disk_cache: bool,
  no_remote_fetch: bool,
}

impl SourceFileFetcher {
  pub fn new(
    deps_cache: DiskCache,
    progress: Progress,
    use_disk_cache: bool,
    no_remote_fetch: bool,
  ) -> std::io::Result<Self> {
    let file_fetcher = Self {
      deps_cache,
      progress,
      source_file_cache: SourceFileCache::default(),
      use_disk_cache,
      no_remote_fetch,
    };

    Ok(file_fetcher)
  }

  fn check_if_supported_scheme(url: &Url) -> Result<(), ErrBox> {
    if !SUPPORTED_URL_SCHEMES.contains(&url.scheme()) {
      return Err(
        DenoError::new(
          ErrorKind::UnsupportedFetchScheme,
          format!("Unsupported scheme \"{}\" for module \"{}\". Supported schemes: {:#?}", url.scheme(), url, SUPPORTED_URL_SCHEMES),
        ).into()
      );
    }

    Ok(())
  }

  /// Required for TS compiler.
  pub fn fetch_source_file(
    self: &Self,
    specifier: &ModuleSpecifier,
  ) -> Result<SourceFile, ErrBox> {
    tokio_util::block_on(self.fetch_source_file_async(specifier))
  }

  pub fn fetch_source_file_async(
    self: &Self,
    specifier: &ModuleSpecifier,
  ) -> Box<SourceFileFuture> {
    let module_url = specifier.as_url().to_owned();
    debug!("fetch_source_file. specifier {} ", &module_url);

    // Check if this file was already fetched and can be retrieved from in-process cache.
    if let Some(source_file) = self.source_file_cache.get(specifier.to_string())
    {
      return Box::new(futures::future::ok(source_file));
    }

    let source_file_cache = self.source_file_cache.clone();
    let specifier_ = specifier.clone();

    let fut = self
      .get_source_file_async(
        &module_url,
        self.use_disk_cache,
        self.no_remote_fetch,
      )
      .then(move |result| {
        let mut out = result.map_err(|err| {
          if err.kind() == ErrorKind::NotFound {
            // For NotFound, change the message to something better.
            DenoError::new(
              ErrorKind::NotFound,
              format!("Cannot resolve module \"{}\"", module_url.to_string()),
            )
            .into()
          } else {
            err
          }
        })?;

        // TODO: move somewhere?
        if out.source_code.starts_with(b"#!") {
          out.source_code = filter_shebang(out.source_code);
        }

        // Cache in-process for subsequent access.
        source_file_cache.set(specifier_.to_string(), out.clone());

        Ok(out)
      });

    Box::new(fut)
  }

  /// This is main method that is responsible for fetching local or remote files.
  ///
  /// If this is a remote module, and it has not yet been cached, the resulting
  /// download will be cached on disk for subsequent access.
  ///
  /// If `use_disk_cache` is true then remote files are fetched from disk cache.
  ///
  /// If `no_remote_fetch` is true then if remote file is not found it disk
  /// cache this method will fail.
  fn get_source_file_async(
    self: &Self,
    module_url: &Url,
    use_disk_cache: bool,
    no_remote_fetch: bool,
  ) -> impl Future<Item = SourceFile, Error = ErrBox> {
    let url_scheme = module_url.scheme();
    let is_local_file = url_scheme == "file";

    if let Err(err) = SourceFileFetcher::check_if_supported_scheme(&module_url)
    {
      return Either::A(futures::future::err(err));
    }

    // Local files are always fetched from disk bypassing cache entirely.
    if is_local_file {
      match self.fetch_local_file(&module_url) {
        Ok(source_file) => {
          return Either::A(futures::future::ok(source_file));
        }
        Err(err) => {
          return Either::A(futures::future::err(err));
        }
      }
    }

    // Fetch remote file and cache on-disk for subsequent access
    Either::B(self.fetch_remote_source_async(
      &module_url,
      use_disk_cache,
      no_remote_fetch,
      10,
    ))
  }

  /// Fetch local source file.
  fn fetch_local_file(
    self: &Self,
    module_url: &Url,
  ) -> Result<SourceFile, ErrBox> {
    let filepath = module_url.to_file_path().expect("File URL expected");

    let source_code = match fs::read(filepath.clone()) {
      Ok(c) => c,
      Err(e) => return Err(e.into()),
    };

    let media_type = map_content_type(&filepath, None);
    Ok(SourceFile {
      url: module_url.clone(),
      filename: filepath,
      media_type,
      source_code,
    })
  }

  /// Fetch cached remote file.
  ///
  /// This is a recursive operation if source file has redirections.
  ///
  /// It will keep reading <filename>.headers.json for information about redirection.
  /// `module_initial_source_name` would be None on first call,
  /// and becomes the name of the very first module that initiates the call
  /// in subsequent recursions.
  ///
  /// AKA if redirection occurs, module_initial_source_name is the source path
  /// that user provides, and the final module_name is the resolved path
  /// after following all redirections.
  fn fetch_cached_remote_source(
    self: &Self,
    module_url: &Url,
  ) -> Result<Option<SourceFile>, ErrBox> {
    let source_code_headers = self.get_source_code_headers(&module_url);
    // If source code headers says that it would redirect elsewhere,
    // (meaning that the source file might not exist; only .headers.json is present)
    // Abort reading attempts to the cached source file and and follow the redirect.
    if let Some(redirect_to) = source_code_headers.redirect_to {
      // E.g.
      // module_name https://import-meta.now.sh/redirect.js
      // filename /Users/kun/Library/Caches/deno/deps/https/import-meta.now.sh/redirect.js
      // redirect_to https://import-meta.now.sh/sub/final1.js
      // real_filename /Users/kun/Library/Caches/deno/deps/https/import-meta.now.sh/sub/final1.js
      // real_module_name = https://import-meta.now.sh/sub/final1.js
      let redirect_url = Url::parse(&redirect_to).expect("Should be valid URL");

      // Recurse.
      // TODO(bartlomieju): I'm pretty sure we should call `fetch_remote_source_async` here.
      // Should we expect that all redirects are cached?
      return self.fetch_cached_remote_source(&redirect_url);
    }

    // No redirect needed or end of redirects.
    // We can try read the file
    let filepath = self
      .deps_cache
      .location
      .join(self.deps_cache.get_cache_filename(&module_url));
    let source_code = match fs::read(filepath.clone()) {
      Err(e) => {
        if e.kind() == std::io::ErrorKind::NotFound {
          return Ok(None);
        } else {
          return Err(e.into());
        }
      }
      Ok(c) => c,
    };
    let media_type = map_content_type(
      &filepath,
      source_code_headers.mime_type.as_ref().map(String::as_str),
    );
    Ok(Some(SourceFile {
      url: module_url.clone(),
      filename: filepath,
      media_type,
      source_code,
    }))
  }

  /// Asynchronously fetch remote source file specified by the URL following redirects.
  fn fetch_remote_source_async(
    self: &Self,
    module_url: &Url,
    use_disk_cache: bool,
    no_remote_fetch: bool,
    redirect_limit: i64,
  ) -> Box<SourceFileFuture> {
    use crate::http_util::FetchOnceResult;

    if redirect_limit < 0 {
      return Box::new(futures::future::err(too_many_redirects()));
    }

    // First try local cache
    if use_disk_cache {
      match self.fetch_cached_remote_source(&module_url) {
        Ok(Some(source_file)) => {
          return Box::new(futures::future::ok(source_file));
        }
        Ok(None) => {
          // there's no cached version
        }
        Err(err) => {
          return Box::new(futures::future::err(err));
        }
      }
    }

    // If file wasn't found in cache check if we can fetch it
    if no_remote_fetch {
      // We can't fetch remote file - bail out
      return Box::new(futures::future::err(
        std::io::Error::new(
          std::io::ErrorKind::NotFound,
          format!(
            "cannot find remote file '{}' in cache",
            module_url.to_string()
          ),
        )
        .into(),
      ));
    }

    let download_job = self.progress.add("Download", &module_url.to_string());

    let module_uri = url_into_uri(&module_url);

    let dir = self.clone();
    let module_url = module_url.clone();

    // Single pass fetch, either yields code or yields redirect.
    let f =
      http_util::fetch_string_once(module_uri).and_then(move |r| match r {
        FetchOnceResult::Redirect(uri) => {
          // If redirects, update module_name and filename for next looped call.
          let new_module_url = Url::parse(&uri.to_string())
            .expect("http::uri::Uri should be parseable as Url");

          dir
            .save_source_code_headers(
              &module_url,
              None,
              Some(new_module_url.to_string()),
            )
            .unwrap();

          // Explicit drop to keep reference alive until future completes.
          drop(download_job);

          // Recurse
          Either::A(dir.fetch_remote_source_async(
            &new_module_url,
            use_disk_cache,
            no_remote_fetch,
            redirect_limit - 1,
          ))
        }
        FetchOnceResult::Code(source, maybe_content_type) => {
          // We land on the code.
          dir
            .save_source_code_headers(
              &module_url,
              maybe_content_type.clone(),
              None,
            )
            .unwrap();

          dir.save_source_code(&module_url, &source).unwrap();

          let filepath = dir
            .deps_cache
            .location
            .join(dir.deps_cache.get_cache_filename(&module_url));

          let media_type = map_content_type(
            &filepath,
            maybe_content_type.as_ref().map(String::as_str),
          );

          let source_file = SourceFile {
            url: module_url.clone(),
            filename: filepath,
            media_type,
            source_code: source.as_bytes().to_owned(),
          };

          // Explicit drop to keep reference alive until future completes.
          drop(download_job);

          Either::B(futures::future::ok(source_file))
        }
      });

    Box::new(f)
  }

  /// Get header metadata associated with a remote file.
  ///
  /// NOTE: chances are that the source file was downloaded due to redirects.
  /// In this case, the headers file provides info about where we should go and get
  /// the file that redirect eventually points to.
  fn get_source_code_headers(self: &Self, url: &Url) -> SourceCodeHeaders {
    let cache_key = self
      .deps_cache
      .get_cache_filename_with_extension(url, "headers.json");

    if let Ok(bytes) = self.deps_cache.get(&cache_key) {
      if let Ok(json_string) = std::str::from_utf8(&bytes) {
        return SourceCodeHeaders::from_json_string(json_string.to_string());
      }
    }

    SourceCodeHeaders::default()
  }

  /// Save contents of downloaded remote file in on-disk cache for subsequent access.
  fn save_source_code(
    self: &Self,
    url: &Url,
    source: &str,
  ) -> std::io::Result<()> {
    let cache_key = self.deps_cache.get_cache_filename(url);

    // May not exist. DON'T unwrap.
    let _ = self.deps_cache.remove(&cache_key);

    self.deps_cache.set(&cache_key, source.as_bytes())
  }

  /// Save headers related to source file to {filename}.headers.json file,
  /// only when there is actually something necessary to save.
  ///
  /// For example, if the extension ".js" already mean JS file and we have
  /// content type of "text/javascript", then we would not save the mime type.
  ///
  /// If nothing needs to be saved, the headers file is not created.
  fn save_source_code_headers(
    self: &Self,
    url: &Url,
    mime_type: Option<String>,
    redirect_to: Option<String>,
  ) -> std::io::Result<()> {
    let cache_key = self
      .deps_cache
      .get_cache_filename_with_extension(url, "headers.json");

    // Remove possibly existing stale .headers.json file.
    // May not exist. DON'T unwrap.
    let _ = self.deps_cache.remove(&cache_key);

    let headers = SourceCodeHeaders {
      mime_type,
      redirect_to,
    };

    let cache_filename = self.deps_cache.get_cache_filename(url);
    if let Ok(maybe_json_string) = headers.to_json_string(&cache_filename) {
      if let Some(json_string) = maybe_json_string {
        return self.deps_cache.set(&cache_key, json_string.as_bytes());
      }
    }

    Ok(())
  }
}

fn map_file_extension(path: &Path) -> msg::MediaType {
  match path.extension() {
    None => msg::MediaType::Unknown,
    Some(os_str) => match os_str.to_str() {
      Some("ts") => msg::MediaType::TypeScript,
      Some("js") => msg::MediaType::JavaScript,
      Some("mjs") => msg::MediaType::JavaScript,
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

fn filter_shebang(bytes: Vec<u8>) -> Vec<u8> {
  let string = str::from_utf8(&bytes).unwrap();
  if let Some(i) = string.find('\n') {
    let (_, rest) = string.split_at(i);
    rest.as_bytes().to_owned()
  } else {
    Vec::new()
  }
}

fn url_into_uri(url: &url::Url) -> http::uri::Uri {
  http::uri::Uri::from_str(&url.to_string())
    .expect("url::Url should be parseable as http::uri::Uri")
}

#[derive(Debug, Default)]
/// Header metadata associated with a particular "symbolic" source code file.
/// (the associated source code file might not be cached, while remaining
/// a user accessible entity through imports (due to redirects)).
pub struct SourceCodeHeaders {
  /// MIME type of the source code.
  pub mime_type: Option<String>,
  /// Where should we actually look for source code.
  /// This should be an absolute path!
  pub redirect_to: Option<String>,
}

static MIME_TYPE: &'static str = "mime_type";
static REDIRECT_TO: &'static str = "redirect_to";

impl SourceCodeHeaders {
  pub fn from_json_string(headers_string: String) -> Self {
    // TODO: use serde for deserialization
    let maybe_headers_json: serde_json::Result<serde_json::Value> =
      serde_json::from_str(&headers_string);

    if let Ok(headers_json) = maybe_headers_json {
      let mime_type = headers_json[MIME_TYPE].as_str().map(String::from);
      let redirect_to = headers_json[REDIRECT_TO].as_str().map(String::from);

      return SourceCodeHeaders {
        mime_type,
        redirect_to,
      };
    }

    SourceCodeHeaders::default()
  }

  // TODO: remove this nonsense `cache_filename` param, this should be
  //  done when instantiating SourceCodeHeaders
  pub fn to_json_string(
    self: &Self,
    cache_filename: &Path,
  ) -> Result<Option<String>, serde_json::Error> {
    // TODO(kevinkassimo): consider introduce serde::Deserialize to make things simpler.
    // This is super ugly at this moment...
    // Had trouble to make serde_derive work: I'm unable to build proc-macro2.
    let mut value_map = serde_json::map::Map::new();

    if let Some(mime_type) = &self.mime_type {
      let resolved_mime_type =
        map_content_type(Path::new(""), Some(mime_type.clone().as_str()));

      // TODO: fix this
      let ext_based_mime_type = map_file_extension(cache_filename);

      // Add mime to headers only when content type is different from extension.
      if ext_based_mime_type == msg::MediaType::Unknown
        || resolved_mime_type != ext_based_mime_type
      {
        value_map.insert(MIME_TYPE.to_string(), json!(mime_type));
      }
    }

    if let Some(redirect_to) = &self.redirect_to {
      value_map.insert(REDIRECT_TO.to_string(), json!(redirect_to));
    }

    if value_map.is_empty() {
      return Ok(None);
    }

    serde_json::to_string(&value_map)
      .and_then(|serialized| Ok(Some(serialized)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fs as deno_fs;
  use tempfile::TempDir;

  impl SourceFileFetcher {
    /// Fetch remote source code.
    fn fetch_remote_source(
      self: &Self,
      module_url: &Url,
      use_disk_cache: bool,
      no_remote_fetch: bool,
      redirect_limit: i64,
    ) -> Result<SourceFile, ErrBox> {
      tokio_util::block_on(self.fetch_remote_source_async(
        module_url,
        use_disk_cache,
        no_remote_fetch,
        redirect_limit,
      ))
    }

    /// Synchronous version of get_source_file_async
    fn get_source_file(
      self: &Self,
      module_url: &Url,
      use_disk_cache: bool,
      no_remote_fetch: bool,
    ) -> Result<SourceFile, ErrBox> {
      tokio_util::block_on(self.get_source_file_async(
        module_url,
        use_disk_cache,
        no_remote_fetch,
      ))
    }
  }

  fn setup_file_fetcher(dir_path: &Path) -> SourceFileFetcher {
    SourceFileFetcher::new(
      DiskCache::new(&dir_path.to_path_buf().join("deps")),
      Progress::new(),
      true,
      false,
    )
    .expect("setup fail")
  }

  fn test_setup() -> (TempDir, SourceFileFetcher) {
    let temp_dir = TempDir::new().expect("tempdir fail");
    let fetcher = setup_file_fetcher(temp_dir.path());
    (temp_dir, fetcher)
  }

  macro_rules! file_url {
    ($path:expr) => {
      if cfg!(target_os = "windows") {
        concat!("file:///C:", $path)
      } else {
        concat!("file://", $path)
      }
    };
  }

  #[test]
  fn test_source_code_headers_get_and_save() {
    let (_temp_dir, fetcher) = test_setup();
    let url = Url::parse("http://example.com/f.js").unwrap();
    let headers_filepath = fetcher.deps_cache.location.join(
      fetcher
        .deps_cache
        .get_cache_filename_with_extension(&url, "headers.json"),
    );

    if let Some(ref parent) = headers_filepath.parent() {
      fs::create_dir_all(parent).unwrap();
    };

    let _ = deno_fs::write_file(
      headers_filepath.as_path(),
      "{\"mime_type\":\"text/javascript\",\"redirect_to\":\"http://example.com/a.js\"}",
      0o666
    );
    let headers = fetcher.get_source_code_headers(&url);

    assert_eq!(headers.mime_type.clone().unwrap(), "text/javascript");
    assert_eq!(
      headers.redirect_to.clone().unwrap(),
      "http://example.com/a.js"
    );

    let _ = fetcher.save_source_code_headers(
      &url,
      Some("text/typescript".to_owned()),
      Some("http://deno.land/a.js".to_owned()),
    );
    let headers2 = fetcher.get_source_code_headers(&url);
    assert_eq!(headers2.mime_type.clone().unwrap(), "text/typescript");
    assert_eq!(
      headers2.redirect_to.clone().unwrap(),
      "http://deno.land/a.js"
    );
  }

  #[test]
  fn test_get_source_code_1() {
    let (temp_dir, fetcher) = test_setup();
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/mod2.ts").unwrap();
      let headers_file_name = fetcher.deps_cache.location.join(
        fetcher
          .deps_cache
          .get_cache_filename_with_extension(&module_url, "headers.json"),
      );

      let result = fetcher.get_source_file(&module_url, true, false);
      assert!(result.is_ok());
      let r = result.unwrap();
      assert_eq!(
        r.source_code,
        "export { printHello } from \"./print_hello.ts\";\n".as_bytes()
      );
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // Should not create .headers.json file due to matching ext
      assert!(fs::read_to_string(&headers_file_name).is_err());

      // Modify .headers.json, write using fs write and read using save_source_code_headers
      let _ =
        fs::write(&headers_file_name, "{ \"mime_type\": \"text/javascript\" }");
      let result2 = fetcher.get_source_file(&module_url, true, false);
      assert!(result2.is_ok());
      let r2 = result2.unwrap();
      assert_eq!(
        r2.source_code,
        "export { printHello } from \"./print_hello.ts\";\n".as_bytes()
      );
      // If get_source_file does not call remote, this should be JavaScript
      // as we modified before! (we do not overwrite .headers.json due to no http fetch)
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        fetcher
          .get_source_code_headers(&module_url)
          .mime_type
          .unwrap(),
        "text/javascript"
      );

      // Modify .headers.json again, but the other way around
      let _ = fetcher.save_source_code_headers(
        &module_url,
        Some("application/json".to_owned()),
        None,
      );
      let result3 = fetcher.get_source_file(&module_url, true, false);
      assert!(result3.is_ok());
      let r3 = result3.unwrap();
      assert_eq!(
        r3.source_code,
        "export { printHello } from \"./print_hello.ts\";\n".as_bytes()
      );
      // If get_source_file does not call remote, this should be JavaScript
      // as we modified before! (we do not overwrite .headers.json due to no http fetch)
      assert_eq!(&(r3.media_type), &msg::MediaType::Json);
      assert!(fs::read_to_string(&headers_file_name)
        .unwrap()
        .contains("application/json"));

      // let's create fresh instance of DenoDir (simulating another freshh Deno process)
      // and don't use cache
      let fetcher = setup_file_fetcher(temp_dir.path());
      let result4 = fetcher.get_source_file(&module_url, false, false);
      assert!(result4.is_ok());
      let r4 = result4.unwrap();
      let expected4 =
        "export { printHello } from \"./print_hello.ts\";\n".as_bytes();
      assert_eq!(r4.source_code, expected4);
      // Now the old .headers.json file should have gone! Resolved back to TypeScript
      assert_eq!(&(r4.media_type), &msg::MediaType::TypeScript);
      assert!(fs::read_to_string(&headers_file_name).is_err());
    });
  }

  #[test]
  fn test_get_source_code_2() {
    let (temp_dir, fetcher) = test_setup();
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/mismatch_ext.ts")
          .unwrap();
      let headers_file_name = fetcher.deps_cache.location.join(
        fetcher
          .deps_cache
          .get_cache_filename_with_extension(&module_url, "headers.json"),
      );

      let result = fetcher.get_source_file(&module_url, true, false);
      assert!(result.is_ok());
      let r = result.unwrap();
      let expected = "export const loaded = true;\n".as_bytes();
      assert_eq!(r.source_code, expected);
      // Mismatch ext with content type, create .headers.json
      assert_eq!(&(r.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        fetcher
          .get_source_code_headers(&module_url)
          .mime_type
          .unwrap(),
        "text/javascript"
      );

      // Modify .headers.json
      let _ = fetcher.save_source_code_headers(
        &module_url,
        Some("text/typescript".to_owned()),
        None,
      );
      let result2 = fetcher.get_source_file(&module_url, true, false);
      assert!(result2.is_ok());
      let r2 = result2.unwrap();
      let expected2 = "export const loaded = true;\n".as_bytes();
      assert_eq!(r2.source_code, expected2);
      // If get_source_file does not call remote, this should be TypeScript
      // as we modified before! (we do not overwrite .headers.json due to no http fetch)
      assert_eq!(&(r2.media_type), &msg::MediaType::TypeScript);
      assert!(fs::read_to_string(&headers_file_name).is_err());

      // let's create fresh instance of DenoDir (simulating another freshh Deno process)
      // and don't use cache
      let fetcher = setup_file_fetcher(temp_dir.path());
      let result3 = fetcher.get_source_file(&module_url, false, false);
      assert!(result3.is_ok());
      let r3 = result3.unwrap();
      let expected3 = "export const loaded = true;\n".as_bytes();
      assert_eq!(r3.source_code, expected3);
      // Now the old .headers.json file should be overwritten back to JavaScript!
      // (due to http fetch)
      assert_eq!(&(r3.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        fetcher
          .get_source_code_headers(&module_url)
          .mime_type
          .unwrap(),
        "text/javascript"
      );
    });
  }

  #[test]
  fn test_get_source_code_multiple_downloads_of_same_file() {
    let (_temp_dir, fetcher) = test_setup();
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let specifier = ModuleSpecifier::resolve_url(
        "http://localhost:4545/tests/subdir/mismatch_ext.ts",
      )
      .unwrap();
      let headers_file_name = fetcher.deps_cache.location.join(
        fetcher.deps_cache.get_cache_filename_with_extension(
          specifier.as_url(),
          "headers.json",
        ),
      );

      // first download
      let result = fetcher.fetch_source_file(&specifier);
      assert!(result.is_ok());

      let result = fs::File::open(&headers_file_name);
      assert!(result.is_ok());
      let headers_file = result.unwrap();
      // save modified timestamp for headers file
      let headers_file_metadata = headers_file.metadata().unwrap();
      let headers_file_modified = headers_file_metadata.modified().unwrap();

      // download file again, it should use already fetched file even though `use_disk_cache` is set to
      // false, this can be verified using source header file creation timestamp (should be
      // the same as after first download)
      let result = fetcher.fetch_source_file(&specifier);
      assert!(result.is_ok());

      let result = fs::File::open(&headers_file_name);
      assert!(result.is_ok());
      let headers_file_2 = result.unwrap();
      // save modified timestamp for headers file
      let headers_file_metadata_2 = headers_file_2.metadata().unwrap();
      let headers_file_modified_2 = headers_file_metadata_2.modified().unwrap();

      assert_eq!(headers_file_modified, headers_file_modified_2);
    });
  }

  #[test]
  fn test_get_source_code_3() {
    let (_temp_dir, fetcher) = test_setup();
    // Test basic follow and headers recording
    tokio_util::init(|| {
      let redirect_module_url =
        Url::parse("http://localhost:4546/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let redirect_source_filepath = fetcher
        .deps_cache
        .location
        .join("http/localhost_PORT4546/tests/subdir/redirects/redirect1.js");
      let redirect_source_filename =
        redirect_source_filepath.to_str().unwrap().to_string();
      let target_module_url =
        Url::parse("http://localhost:4545/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let redirect_target_filepath = fetcher
        .deps_cache
        .location
        .join("http/localhost_PORT4545/tests/subdir/redirects/redirect1.js");
      let redirect_target_filename =
        redirect_target_filepath.to_str().unwrap().to_string();

      let mod_meta = fetcher
        .get_source_file(&redirect_module_url, true, false)
        .unwrap();
      // File that requires redirection is not downloaded.
      assert!(fs::read_to_string(&redirect_source_filename).is_err());
      // ... but its .headers.json is created.
      let redirect_source_headers =
        fetcher.get_source_code_headers(&redirect_module_url);
      assert_eq!(
        redirect_source_headers.redirect_to.unwrap(),
        "http://localhost:4545/tests/subdir/redirects/redirect1.js"
      );
      // The target of redirection is downloaded instead.
      assert_eq!(
        fs::read_to_string(&redirect_target_filename).unwrap(),
        "export const redirect = 1;\n"
      );
      let redirect_target_headers =
        fetcher.get_source_code_headers(&target_module_url);
      assert!(redirect_target_headers.redirect_to.is_none());

      // Examine the meta result.
      assert_eq!(mod_meta.url.clone(), target_module_url);
    });
  }

  #[test]
  fn test_get_source_code_4() {
    let (_temp_dir, fetcher) = test_setup();
    // Test double redirects and headers recording
    tokio_util::init(|| {
      let double_redirect_url =
        Url::parse("http://localhost:4548/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let double_redirect_path = fetcher
        .deps_cache
        .location
        .join("http/localhost_PORT4548/tests/subdir/redirects/redirect1.js");

      let redirect_url =
        Url::parse("http://localhost:4546/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let redirect_path = fetcher
        .deps_cache
        .location
        .join("http/localhost_PORT4546/tests/subdir/redirects/redirect1.js");

      let target_url =
        Url::parse("http://localhost:4545/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let target_path = fetcher
        .deps_cache
        .location
        .join("http/localhost_PORT4545/tests/subdir/redirects/redirect1.js");

      let mod_meta = fetcher
        .get_source_file(&double_redirect_url, true, false)
        .unwrap();

      assert!(fs::read_to_string(&double_redirect_path).is_err());
      assert!(fs::read_to_string(&redirect_path).is_err());

      let double_redirect_headers =
        fetcher.get_source_code_headers(&double_redirect_url);
      assert_eq!(
        double_redirect_headers.redirect_to.unwrap(),
        redirect_url.to_string()
      );
      let redirect_headers = fetcher.get_source_code_headers(&redirect_url);
      assert_eq!(
        redirect_headers.redirect_to.unwrap(),
        target_url.to_string()
      );

      // The target of redirection is downloaded instead.
      assert_eq!(
        fs::read_to_string(&target_path).unwrap(),
        "export const redirect = 1;\n"
      );
      let redirect_target_headers =
        fetcher.get_source_code_headers(&target_url);
      assert!(redirect_target_headers.redirect_to.is_none());

      // Examine the meta result.
      assert_eq!(mod_meta.url.clone(), target_url);
    });
  }

  #[test]
  fn test_get_source_code_5() {
    let (_temp_dir, fetcher) = test_setup();
    // Test that redirect target is not downloaded twice for different redirect source.
    tokio_util::init(|| {
      let double_redirect_url =
        Url::parse("http://localhost:4548/tests/subdir/redirects/redirect1.js")
          .unwrap();

      let redirect_url =
        Url::parse("http://localhost:4546/tests/subdir/redirects/redirect1.js")
          .unwrap();

      let target_path = fetcher
        .deps_cache
        .location
        .join("http/localhost_PORT4545/tests/subdir/redirects/redirect1.js");

      fetcher
        .get_source_file(&double_redirect_url, true, false)
        .unwrap();

      let result = fs::File::open(&target_path);
      assert!(result.is_ok());
      let file = result.unwrap();
      // save modified timestamp for headers file of redirect target
      let file_metadata = file.metadata().unwrap();
      let file_modified = file_metadata.modified().unwrap();

      // When another file is fetched that also point to redirect target, then redirect target
      // shouldn't be downloaded again. It can be verified using source header file creation
      // timestamp (should be the same as after first `get_source_file`)
      fetcher.get_source_file(&redirect_url, true, false).unwrap();

      let result = fs::File::open(&target_path);
      assert!(result.is_ok());
      let file_2 = result.unwrap();
      // save modified timestamp for headers file
      let file_metadata_2 = file_2.metadata().unwrap();
      let file_modified_2 = file_metadata_2.modified().unwrap();

      assert_eq!(file_modified, file_modified_2);
    });
  }

  #[test]
  fn test_get_source_code_6() {
    let (_temp_dir, fetcher) = test_setup();
    // Test that redirections can be limited
    tokio_util::init(|| {
      let double_redirect_url =
        Url::parse("http://localhost:4548/tests/subdir/redirects/redirect1.js")
          .unwrap();

      let result =
        fetcher.fetch_remote_source(&double_redirect_url, false, false, 2);
      assert!(result.is_ok());
      let result =
        fetcher.fetch_remote_source(&double_redirect_url, false, false, 1);
      assert!(result.is_err());
      let err = result.err().unwrap();
      assert_eq!(err.kind(), ErrorKind::TooManyRedirects);
    });
  }

  #[test]
  fn test_get_source_code_no_fetch() {
    let (_temp_dir, fetcher) = test_setup();
    tokio_util::init(|| {
      let module_url =
        Url::parse("http://localhost:4545/tests/002_hello.ts").unwrap();

      // file hasn't been cached before and remote downloads are not allowed
      let result = fetcher.get_source_file(&module_url, true, true);
      assert!(result.is_err());
      let err = result.err().unwrap();
      assert_eq!(err.kind(), ErrorKind::NotFound);

      // download and cache file
      let result = fetcher.get_source_file(&module_url, true, false);
      assert!(result.is_ok());

      // module is already cached, should be ok even with `no_remote_fetch`
      let result = fetcher.get_source_file(&module_url, true, true);
      assert!(result.is_ok());
    });
  }

  #[test]
  fn test_fetch_source_async_1() {
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let (_temp_dir, fetcher) = test_setup();
      let module_url =
        Url::parse("http://127.0.0.1:4545/tests/subdir/mt_video_mp2t.t3.ts")
          .unwrap();
      let headers_file_name = fetcher.deps_cache.location.join(
        fetcher
          .deps_cache
          .get_cache_filename_with_extension(&module_url, "headers.json"),
      );

      let result = tokio_util::block_on(fetcher.fetch_remote_source_async(
        &module_url,
        false,
        false,
        10,
      ));
      assert!(result.is_ok());
      let r = result.unwrap();
      assert_eq!(r.source_code, b"export const loaded = true;\n");
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // matching ext, no .headers.json file created
      assert!(fs::read_to_string(&headers_file_name).is_err());

      // Modify .headers.json, make sure read from local
      let _ = fetcher.save_source_code_headers(
        &module_url,
        Some("text/javascript".to_owned()),
        None,
      );
      let result2 = fetcher.fetch_cached_remote_source(&module_url);
      assert!(result2.is_ok());
      let r2 = result2.unwrap().unwrap();
      assert_eq!(r2.source_code, b"export const loaded = true;\n");
      // Not MediaType::TypeScript due to .headers.json modification
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
    });
  }

  #[test]
  fn test_fetch_source_1() {
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let (_temp_dir, fetcher) = test_setup();
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/mt_video_mp2t.t3.ts")
          .unwrap();
      let headers_file_name = fetcher.deps_cache.location.join(
        fetcher
          .deps_cache
          .get_cache_filename_with_extension(&module_url, "headers.json"),
      );

      let result = fetcher.fetch_remote_source(&module_url, false, false, 10);
      assert!(result.is_ok());
      let r = result.unwrap();
      assert_eq!(r.source_code, "export const loaded = true;\n".as_bytes());
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // matching ext, no .headers.json file created
      assert!(fs::read_to_string(&headers_file_name).is_err());

      // Modify .headers.json, make sure read from local
      let _ = fetcher.save_source_code_headers(
        &module_url,
        Some("text/javascript".to_owned()),
        None,
      );
      let result2 = fetcher.fetch_cached_remote_source(&module_url);
      assert!(result2.is_ok());
      let r2 = result2.unwrap().unwrap();
      assert_eq!(r2.source_code, "export const loaded = true;\n".as_bytes());
      // Not MediaType::TypeScript due to .headers.json modification
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
    });
  }

  #[test]
  fn test_fetch_source_2() {
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let (_temp_dir, fetcher) = test_setup();
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/no_ext").unwrap();
      let result = fetcher.fetch_remote_source(&module_url, false, false, 10);
      assert!(result.is_ok());
      let r = result.unwrap();
      assert_eq!(r.source_code, "export const loaded = true;\n".as_bytes());
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // no ext, should create .headers.json file
      assert_eq!(
        fetcher
          .get_source_code_headers(&module_url)
          .mime_type
          .unwrap(),
        "text/typescript"
      );

      let module_url_2 =
        Url::parse("http://localhost:4545/tests/subdir/mismatch_ext.ts")
          .unwrap();
      let result_2 =
        fetcher.fetch_remote_source(&module_url_2, false, false, 10);
      assert!(result_2.is_ok());
      let r2 = result_2.unwrap();
      assert_eq!(r2.source_code, "export const loaded = true;\n".as_bytes());
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
      // mismatch ext, should create .headers.json file
      assert_eq!(
        fetcher
          .get_source_code_headers(&module_url_2)
          .mime_type
          .unwrap(),
        "text/javascript"
      );

      // test unknown extension
      let module_url_3 =
        Url::parse("http://localhost:4545/tests/subdir/unknown_ext.deno")
          .unwrap();
      let result_3 =
        fetcher.fetch_remote_source(&module_url_3, false, false, 10);
      assert!(result_3.is_ok());
      let r3 = result_3.unwrap();
      assert_eq!(r3.source_code, "export const loaded = true;\n".as_bytes());
      assert_eq!(&(r3.media_type), &msg::MediaType::TypeScript);
      // unknown ext, should create .headers.json file
      assert_eq!(
        fetcher
          .get_source_code_headers(&module_url_3)
          .mime_type
          .unwrap(),
        "text/typescript"
      );
    });
  }

  #[test]
  fn test_fetch_source_file() {
    let (_temp_dir, fetcher) = test_setup();

    tokio_util::init(|| {
      // Test failure case.
      let specifier =
        ModuleSpecifier::resolve_url(file_url!("/baddir/hello.ts")).unwrap();
      let r = fetcher.fetch_source_file(&specifier);
      assert!(r.is_err());

      // Assuming cwd is the deno repo root.
      let specifier =
        ModuleSpecifier::resolve_url_or_path("js/main.ts").unwrap();
      let r = fetcher.fetch_source_file(&specifier);
      assert!(r.is_ok());
    })
  }

  #[test]
  fn test_fetch_source_file_1() {
    /*recompile ts file*/
    let (_temp_dir, fetcher) = test_setup();

    tokio_util::init(|| {
      // Test failure case.
      let specifier =
        ModuleSpecifier::resolve_url(file_url!("/baddir/hello.ts")).unwrap();
      let r = fetcher.fetch_source_file(&specifier);
      assert!(r.is_err());

      // Assuming cwd is the deno repo root.
      let specifier =
        ModuleSpecifier::resolve_url_or_path("js/main.ts").unwrap();
      let r = fetcher.fetch_source_file(&specifier);
      assert!(r.is_ok());
    })
  }

  #[test]
  fn test_resolve_module_3() {
    // unsupported schemes
    let test_cases = [
      "ftp://localhost:4545/testdata/subdir/print_hello.ts",
      "blob:https://whatwg.org/d0360e2f-caee-469f-9a2f-87d5b0456f6f",
    ];

    for &test in test_cases.iter() {
      let url = Url::parse(test).unwrap();
      assert_eq!(
        SourceFileFetcher::check_if_supported_scheme(&url)
          .unwrap_err()
          .kind(),
        ErrorKind::UnsupportedFetchScheme
      );
    }
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
    assert_eq!(filter_shebang(b"#!"[..].to_owned()), b"");
    assert_eq!(
      filter_shebang("#!\n\n".as_bytes().to_owned()),
      "\n\n".as_bytes()
    );
    let code = "#!/usr/bin/env deno\nconsole.log('hello');\n"
      .as_bytes()
      .to_owned();
    assert_eq!(filter_shebang(code), "\nconsole.log('hello');\n".as_bytes());
  }
}
