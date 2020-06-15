// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use crate::http_cache::HttpCache;
use crate::http_util;
use crate::http_util::create_http_client;
use crate::http_util::FetchOnceResult;
use crate::msg;
use crate::op_error::OpError;
use crate::permissions::Permissions;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use futures::future::FutureExt;
use log::info;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::result::Result;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;
use url::Url;

/// Structure representing local or remote file.
///
/// In case of remote file `url` might be different than originally requested URL, if so
/// `redirect_source_url` will contain original URL and `url` will be equal to final location.
#[derive(Debug, Clone)]
pub struct SourceFile {
  pub url: Url,
  pub filename: PathBuf,
  pub types_url: Option<Url>,
  pub types_header: Option<String>,
  pub media_type: msg::MediaType,
  pub source_code: Vec<u8>,
}

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

#[derive(Clone)]
pub struct SourceFileFetcher {
  source_file_cache: SourceFileCache,
  cache_blocklist: Vec<String>,
  use_disk_cache: bool,
  no_remote: bool,
  cached_only: bool,
  http_client: reqwest::Client,
  // This field is public only to expose it's location
  pub http_cache: HttpCache,
}

impl SourceFileFetcher {
  pub fn new(
    http_cache: HttpCache,
    use_disk_cache: bool,
    cache_blocklist: Vec<String>,
    no_remote: bool,
    cached_only: bool,
    ca_file: Option<String>,
  ) -> Result<Self, ErrBox> {
    let file_fetcher = Self {
      http_cache,
      source_file_cache: SourceFileCache::default(),
      cache_blocklist,
      use_disk_cache,
      no_remote,
      cached_only,
      http_client: create_http_client(ca_file)?,
    };

    Ok(file_fetcher)
  }

  pub fn check_if_supported_scheme(url: &Url) -> Result<(), ErrBox> {
    if !SUPPORTED_URL_SCHEMES.contains(&url.scheme()) {
      return Err(
        OpError::other(
          format!("Unsupported scheme \"{}\" for module \"{}\". Supported schemes: {:#?}", url.scheme(), url, SUPPORTED_URL_SCHEMES),
        ).into()
      );
    }

    Ok(())
  }

  /// Required for TS compiler and source maps.
  pub fn fetch_cached_source_file(
    &self,
    specifier: &ModuleSpecifier,
    permissions: Permissions,
  ) -> Option<SourceFile> {
    let maybe_source_file = self.source_file_cache.get(specifier.to_string());

    if maybe_source_file.is_some() {
      return maybe_source_file;
    }

    // If file is not in memory cache check if it can be found
    // in local cache - which effectively means trying to fetch
    // using "--cached-only" flag.
    // It should be safe to for caller block on this
    // future, because it doesn't actually do any asynchronous
    // action in that path.
    if let Ok(maybe_source_file) =
      self.get_source_file_from_local_cache(specifier.as_url(), &permissions)
    {
      return maybe_source_file;
    }

    None
  }

  /// Save a given source file into cache.
  /// Allows injection of files that normally would not present
  /// in filesystem.
  /// This is useful when e.g. TS compiler retrieves a custom file
  /// under a dummy specifier.
  pub fn save_source_file_in_cache(
    &self,
    specifier: &ModuleSpecifier,
    file: SourceFile,
  ) {
    self.source_file_cache.set(specifier.to_string(), file);
  }

  pub async fn fetch_source_file(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    permissions: Permissions,
  ) -> Result<SourceFile, ErrBox> {
    let module_url = specifier.as_url().to_owned();
    debug!(
      "fetch_source_file specifier: {} maybe_referrer: {:#?}",
      &module_url,
      maybe_referrer.as_ref()
    );

    // Check if this file was already fetched and can be retrieved from in-process cache.
    let maybe_cached_file = self.source_file_cache.get(specifier.to_string());
    if let Some(source_file) = maybe_cached_file {
      return Ok(source_file);
    }

    let source_file_cache = self.source_file_cache.clone();
    let specifier_ = specifier.clone();

    let result = self
      .get_source_file(
        &module_url,
        self.use_disk_cache,
        self.no_remote,
        self.cached_only,
        &permissions,
      )
      .await;

    match result {
      Ok(mut file) => {
        // TODO: move somewhere?
        if file.source_code.starts_with(b"#!") {
          file.source_code = filter_shebang(file.source_code);
        }

        // Cache in-process for subsequent access.
        source_file_cache.set(specifier_.to_string(), file.clone());

        Ok(file)
      }
      Err(err) => {
        // FIXME(bartlomieju): rewrite this whole block

        // FIXME(bartlomieju): very ugly
        let mut is_not_found = false;
        if let Some(e) = err.downcast_ref::<std::io::Error>() {
          if e.kind() == std::io::ErrorKind::NotFound {
            is_not_found = true;
          }
        }
        let referrer_suffix = if let Some(referrer) = maybe_referrer {
          format!(r#" from "{}""#, referrer)
        } else {
          "".to_owned()
        };
        // Hack: Check error message for "--cached-only" because the kind
        // conflicts with other errors.
        let err = if err.to_string().contains("--cached-only") {
          let msg = format!(
            r#"Cannot find module "{}"{} in cache, --cached-only is specified"#,
            module_url, referrer_suffix
          );
          OpError::not_found(msg).into()
        } else if is_not_found {
          let msg = format!(
            r#"Cannot resolve module "{}"{}"#,
            module_url, referrer_suffix
          );
          OpError::not_found(msg).into()
        } else {
          err
        };
        Err(err)
      }
    }
  }

  fn get_source_file_from_local_cache(
    &self,
    module_url: &Url,
    permissions: &Permissions,
  ) -> Result<Option<SourceFile>, ErrBox> {
    let url_scheme = module_url.scheme();
    let is_local_file = url_scheme == "file";
    SourceFileFetcher::check_if_supported_scheme(&module_url)?;

    // Local files are always fetched from disk bypassing cache entirely.
    if is_local_file {
      return self.fetch_local_file(&module_url, permissions).map(Some);
    }

    self.fetch_cached_remote_source(&module_url, 10)
  }

  /// This is main method that is responsible for fetching local or remote files.
  ///
  /// If this is a remote module, and it has not yet been cached, the resulting
  /// download will be cached on disk for subsequent access.
  ///
  /// If `use_disk_cache` is true then remote files are fetched from disk cache.
  ///
  /// If `no_remote` is true then this method will fail for remote files.
  ///
  /// If `cached_only` is true then this method will fail for remote files
  /// not already cached.
  async fn get_source_file(
    &self,
    module_url: &Url,
    use_disk_cache: bool,
    no_remote: bool,
    cached_only: bool,
    permissions: &Permissions,
  ) -> Result<SourceFile, ErrBox> {
    let url_scheme = module_url.scheme();
    let is_local_file = url_scheme == "file";
    SourceFileFetcher::check_if_supported_scheme(&module_url)?;

    // Local files are always fetched from disk bypassing cache entirely.
    if is_local_file {
      return self.fetch_local_file(&module_url, permissions);
    }

    // The file is remote, fail if `no_remote` is true.
    if no_remote {
      let e = std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
          "Not allowed to get remote file '{}'",
          module_url.to_string()
        ),
      );
      return Err(e.into());
    }

    // Fetch remote file and cache on-disk for subsequent access
    self
      .fetch_remote_source(
        &module_url,
        use_disk_cache,
        cached_only,
        10,
        permissions,
      )
      .await
  }

  /// Fetch local source file.
  fn fetch_local_file(
    &self,
    module_url: &Url,
    permissions: &Permissions,
  ) -> Result<SourceFile, ErrBox> {
    let filepath = module_url.to_file_path().map_err(|()| {
      ErrBox::from(OpError::uri_error(
        "File URL contains invalid path".to_owned(),
      ))
    })?;

    permissions.check_read(&filepath)?;
    let source_code = match fs::read(filepath.clone()) {
      Ok(c) => c,
      Err(e) => return Err(e.into()),
    };

    let media_type = map_content_type(&filepath, None);
    let types_url = match media_type {
      msg::MediaType::JavaScript | msg::MediaType::JSX => {
        get_types_url(&module_url, &source_code, None)
      }
      _ => None,
    };
    Ok(SourceFile {
      url: module_url.clone(),
      filename: filepath,
      media_type,
      source_code,
      types_url,
      types_header: None,
    })
  }

  /// Fetch cached remote file.
  ///
  /// This is a recursive operation if source file has redirections.
  ///
  /// It will keep reading <filename>.metadata.json for information about redirection.
  /// `module_initial_source_name` would be None on first call,
  /// and becomes the name of the very first module that initiates the call
  /// in subsequent recursions.
  ///
  /// AKA if redirection occurs, module_initial_source_name is the source path
  /// that user provides, and the final module_name is the resolved path
  /// after following all redirections.
  fn fetch_cached_remote_source(
    &self,
    module_url: &Url,
    redirect_limit: i64,
  ) -> Result<Option<SourceFile>, ErrBox> {
    if redirect_limit < 0 {
      let e = OpError::http("too many redirects".to_string());
      return Err(e.into());
    }

    let result = self.http_cache.get(&module_url);
    let result = match result {
      Err(e) => {
        if let Some(e) = e.downcast_ref::<std::io::Error>() {
          if e.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
          }
        }
        return Err(e);
      }
      Ok(c) => c,
    };

    let (mut source_file, headers) = result;
    if let Some(redirect_to) = headers.get("location") {
      let redirect_url = match Url::parse(redirect_to) {
        Ok(redirect_url) => redirect_url,
        Err(url::ParseError::RelativeUrlWithoutBase) => {
          let mut url = module_url.clone();
          url.set_path(redirect_to);
          url
        }
        Err(e) => {
          return Err(e.into());
        }
      };
      return self
        .fetch_cached_remote_source(&redirect_url, redirect_limit - 1);
    }

    let mut source_code = Vec::new();
    source_file.read_to_end(&mut source_code)?;

    let cache_filename = self.http_cache.get_cache_filename(module_url);
    let fake_filepath = PathBuf::from(module_url.path());
    let media_type = map_content_type(
      &fake_filepath,
      headers.get("content-type").map(|e| e.as_str()),
    );
    let types_header = headers.get("x-typescript-types").map(|e| e.to_string());
    let types_url = match media_type {
      msg::MediaType::JavaScript | msg::MediaType::JSX => get_types_url(
        &module_url,
        &source_code,
        headers.get("x-typescript-types").map(|e| e.as_str()),
      ),
      _ => None,
    };
    Ok(Some(SourceFile {
      url: module_url.clone(),
      filename: cache_filename,
      media_type,
      source_code,
      types_url,
      types_header,
    }))
  }

  /// Asynchronously fetch remote source file specified by the URL following redirects.
  ///
  /// Note that this is a recursive method so it can't be "async", but rather return
  /// Pin<Box<..>>.
  fn fetch_remote_source(
    &self,
    module_url: &Url,
    use_disk_cache: bool,
    cached_only: bool,
    redirect_limit: i64,
    permissions: &Permissions,
  ) -> Pin<Box<dyn Future<Output = Result<SourceFile, ErrBox>>>> {
    if redirect_limit < 0 {
      let e = OpError::http("too many redirects".to_string());
      return futures::future::err(e.into()).boxed_local();
    }

    if let Err(e) = permissions.check_net_url(&module_url) {
      return futures::future::err(e.into()).boxed_local();
    }

    let is_blocked =
      check_cache_blocklist(module_url, self.cache_blocklist.as_ref());
    // First try local cache
    if use_disk_cache && !is_blocked {
      match self.fetch_cached_remote_source(&module_url, redirect_limit) {
        Ok(Some(source_file)) => {
          return futures::future::ok(source_file).boxed_local();
        }
        Ok(None) => {
          // there's no cached version
        }
        Err(err) => {
          return futures::future::err(err).boxed_local();
        }
      }
    }

    // If file wasn't found in cache check if we can fetch it
    if cached_only {
      // We can't fetch remote file - bail out
      return futures::future::err(
        std::io::Error::new(
          std::io::ErrorKind::NotFound,
          format!(
            "Cannot find remote file '{}' in cache, --cached-only is specified",
            module_url.to_string()
          ),
        )
        .into(),
      )
      .boxed_local();
    }

    info!(
      "{} {}",
      colors::green("Download".to_string()),
      module_url.to_string()
    );

    let dir = self.clone();
    let module_url = module_url.clone();
    let module_etag = match self.http_cache.get(&module_url) {
      Ok((_, headers)) => headers.get("etag").map(String::from),
      Err(_) => None,
    };
    let permissions = permissions.clone();
    let http_client = self.http_client.clone();
    // Single pass fetch, either yields code or yields redirect.
    let f = async move {
      match http_util::fetch_once(http_client, &module_url, module_etag).await?
      {
        FetchOnceResult::NotModified => {
          let source_file =
            dir.fetch_cached_remote_source(&module_url, 10)?.unwrap();

          Ok(source_file)
        }
        FetchOnceResult::Redirect(new_module_url, headers) => {
          // If redirects, update module_name and filename for next looped call.
          dir.http_cache.set(&module_url, headers, &[])?;

          // Recurse
          dir
            .fetch_remote_source(
              &new_module_url,
              use_disk_cache,
              cached_only,
              redirect_limit - 1,
              &permissions,
            )
            .await
        }
        FetchOnceResult::Code(source, headers) => {
          // We land on the code.
          dir.http_cache.set(&module_url, headers.clone(), &source)?;

          let cache_filepath = dir.http_cache.get_cache_filename(&module_url);
          // Used to sniff out content type from file extension - probably to be removed
          let fake_filepath = PathBuf::from(module_url.path());
          let media_type = map_content_type(
            &fake_filepath,
            headers.get("content-type").map(String::as_str),
          );

          let types_header =
            headers.get("x-typescript-types").map(String::to_string);
          let types_url = match media_type {
            msg::MediaType::JavaScript | msg::MediaType::JSX => get_types_url(
              &module_url,
              &source,
              headers.get("x-typescript-types").map(String::as_str),
            ),
            _ => None,
          };

          let source_file = SourceFile {
            url: module_url.clone(),
            filename: cache_filepath,
            media_type,
            source_code: source,
            types_url,
            types_header,
          };

          Ok(source_file)
        }
      }
    };

    f.boxed_local()
  }
}

pub fn map_file_extension(path: &Path) -> msg::MediaType {
  match path.extension() {
    None => msg::MediaType::Unknown,
    Some(os_str) => match os_str.to_str() {
      Some("ts") => msg::MediaType::TypeScript,
      Some("tsx") => msg::MediaType::TSX,
      Some("js") => msg::MediaType::JavaScript,
      Some("jsx") => msg::MediaType::JSX,
      Some("mjs") => msg::MediaType::JavaScript,
      Some("cjs") => msg::MediaType::JavaScript,
      Some("json") => msg::MediaType::Json,
      Some("wasm") => msg::MediaType::Wasm,
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
        | "application/x-typescript" => {
          map_js_like_extension(path, msg::MediaType::TypeScript)
        }
        "application/javascript"
        | "text/javascript"
        | "application/ecmascript"
        | "text/ecmascript"
        | "application/x-javascript"
        | "application/node" => {
          map_js_like_extension(path, msg::MediaType::JavaScript)
        }
        "application/json" | "text/json" => msg::MediaType::Json,
        "application/wasm" => msg::MediaType::Wasm,
        // Handle plain and possibly webassembly
        "text/plain" | "application/octet-stream" => map_file_extension(path),
        _ => {
          debug!("unknown content type: {}", content_type);
          msg::MediaType::Unknown
        }
      }
    }
    None => map_file_extension(path),
  }
}

fn map_js_like_extension(
  path: &Path,
  default: msg::MediaType,
) -> msg::MediaType {
  match path.extension() {
    None => default,
    Some(os_str) => match os_str.to_str() {
      None => default,
      Some("jsx") => msg::MediaType::JSX,
      Some("tsx") => msg::MediaType::TSX,
      Some(_) => default,
    },
  }
}

/// Take a module URL and source code and determines if the source code contains
/// a type directive, and if so, returns the parsed URL for that type directive.
fn get_types_url(
  module_url: &Url,
  source_code: &[u8],
  maybe_types_header: Option<&str>,
) -> Option<Url> {
  lazy_static! {
    /// Matches reference type directives in strings, which provide
    /// type files that should be used by the compiler instead of the
    /// JavaScript file.
    static ref DIRECTIVE_TYPES: Regex = Regex::new(
      r#"(?m)^/{3}\s*<reference\s+types\s*=\s*["']([^"']+)["']\s*/>"#
    )
    .unwrap();
  }

  match maybe_types_header {
    Some(types_header) => match Url::parse(&types_header) {
      Ok(url) => Some(url),
      _ => Some(module_url.join(&types_header).unwrap()),
    },
    _ => match DIRECTIVE_TYPES.captures(str::from_utf8(source_code).unwrap()) {
      Some(cap) => {
        let val = cap.get(1).unwrap().as_str();
        match Url::parse(&val) {
          Ok(url) => Some(url),
          _ => Some(module_url.join(&val).unwrap()),
        }
      }
      _ => None,
    },
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

fn check_cache_blocklist(url: &Url, black_list: &[String]) -> bool {
  let mut url_without_fragmets = url.clone();
  url_without_fragmets.set_fragment(None);
  if black_list.contains(&String::from(url_without_fragmets.as_str())) {
    return true;
  }
  let mut url_without_query_strings = url_without_fragmets;
  url_without_query_strings.set_query(None);
  let mut path_buf = PathBuf::from(url_without_query_strings.as_str());
  loop {
    if black_list.contains(&String::from(path_buf.to_str().unwrap())) {
      return true;
    }
    if !path_buf.pop() {
      break;
    }
  }
  false
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
  /// ETag of the remote source file
  pub etag: Option<String>,
  /// X-TypeScript-Types defines the location of a .d.ts file
  pub x_typescript_types: Option<String>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  fn setup_file_fetcher(dir_path: &Path) -> SourceFileFetcher {
    SourceFileFetcher::new(
      HttpCache::new(&dir_path.to_path_buf().join("deps")),
      true,
      vec![],
      false,
      false,
      None,
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
  fn test_cache_blocklist() {
    let args = crate::flags::resolve_urls(vec![
      String::from("http://deno.land/std"),
      String::from("http://github.com/example/mod.ts"),
      String::from("http://fragment.com/mod.ts#fragment"),
      String::from("http://query.com/mod.ts?foo=bar"),
      String::from("http://queryandfragment.com/mod.ts?foo=bar#fragment"),
    ]);

    let u: Url = "http://deno.land/std/fs/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://github.com/example/file.ts".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), false);

    let u: Url = "http://github.com/example/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://github.com/example/mod.ts?foo=bar".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://github.com/example/mod.ts#fragment".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://fragment.com/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://query.com/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), false);

    let u: Url = "http://fragment.com/mod.ts#fragment".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://query.com/mod.ts?foo=bar".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://queryandfragment.com/mod.ts".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), false);

    let u: Url = "http://queryandfragment.com/mod.ts?foo=bar"
      .parse()
      .unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://queryandfragment.com/mod.ts#fragment"
      .parse()
      .unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), false);

    let u: Url = "http://query.com/mod.ts?foo=bar#fragment".parse().unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);

    let u: Url = "http://fragment.com/mod.ts?foo=bar#fragment"
      .parse()
      .unwrap();
    assert_eq!(check_cache_blocklist(&u, &args), true);
  }

  #[test]
  fn test_fetch_local_file_no_panic() {
    let (_temp_dir, fetcher) = test_setup();
    if cfg!(windows) {
      // Should fail: missing drive letter.
      let u = Url::parse("file:///etc/passwd").unwrap();
      fetcher
        .fetch_local_file(&u, &Permissions::allow_all())
        .unwrap_err();
    } else {
      // Should fail: local network paths are not supported on unix.
      let u = Url::parse("file://server/etc/passwd").unwrap();
      fetcher
        .fetch_local_file(&u, &Permissions::allow_all())
        .unwrap_err();
    }
  }

  #[tokio::test]
  async fn test_get_source_code_1() {
    let http_server_guard = crate::test_util::http_server();
    let (temp_dir, fetcher) = test_setup();
    let fetcher_1 = fetcher.clone();
    let fetcher_2 = fetcher.clone();
    let module_url =
      Url::parse("http://localhost:4545/cli/tests/subdir/mod2.ts").unwrap();
    let module_url_1 = module_url.clone();
    let module_url_2 = module_url.clone();

    let cache_filename = fetcher.http_cache.get_cache_filename(&module_url);

    let result = fetcher
      .get_source_file(
        &module_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let r = result.unwrap();
    assert_eq!(
      r.source_code,
      &b"export { printHello } from \"./print_hello.ts\";\n"[..]
    );
    assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);

    let mut metadata =
      crate::http_cache::Metadata::read(&cache_filename).unwrap();

    // Modify .headers.json, write using fs write
    metadata.headers = HashMap::new();
    metadata
      .headers
      .insert("content-type".to_string(), "text/javascript".to_string());
    metadata.write(&cache_filename).unwrap();

    let result2 = fetcher_1
      .get_source_file(
        &module_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result2.is_ok());
    let r2 = result2.unwrap();
    assert_eq!(
      r2.source_code,
      &b"export { printHello } from \"./print_hello.ts\";\n"[..]
    );
    // If get_source_file does not call remote, this should be JavaScript
    // as we modified before! (we do not overwrite .headers.json due to no http fetch)
    assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
    let (_, headers) = fetcher_2.http_cache.get(&module_url_1).unwrap();

    assert_eq!(headers.get("content-type").unwrap(), "text/javascript");

    // Modify .headers.json again, but the other way around
    metadata.headers = HashMap::new();
    metadata
      .headers
      .insert("content-type".to_string(), "application/json".to_string());
    metadata.write(&cache_filename).unwrap();

    let result3 = fetcher_2
      .get_source_file(
        &module_url_1,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result3.is_ok());
    let r3 = result3.unwrap();
    assert_eq!(
      r3.source_code,
      &b"export { printHello } from \"./print_hello.ts\";\n"[..]
    );
    // If get_source_file does not call remote, this should be JavaScript
    // as we modified before! (we do not overwrite .headers.json due to no http fetch)
    assert_eq!(&(r3.media_type), &msg::MediaType::Json);
    let metadata = crate::http_cache::Metadata::read(&cache_filename).unwrap();
    assert_eq!(
      metadata.headers.get("content-type").unwrap(),
      "application/json"
    );

    // let's create fresh instance of DenoDir (simulating another freshh Deno process)
    // and don't use cache
    let fetcher = setup_file_fetcher(temp_dir.path());
    let result4 = fetcher
      .get_source_file(
        &module_url_2,
        false,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result4.is_ok());
    let r4 = result4.unwrap();
    let expected4 = &b"export { printHello } from \"./print_hello.ts\";\n"[..];
    assert_eq!(r4.source_code, expected4);
    // Resolved back to TypeScript
    assert_eq!(&(r4.media_type), &msg::MediaType::TypeScript);

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_code_2() {
    let http_server_guard = crate::test_util::http_server();
    let (temp_dir, fetcher) = test_setup();
    let module_url =
      Url::parse("http://localhost:4545/cli/tests/subdir/mismatch_ext.ts")
        .unwrap();
    let module_url_1 = module_url.clone();

    let cache_filename = fetcher.http_cache.get_cache_filename(&module_url);

    let result = fetcher
      .get_source_file(
        &module_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let r = result.unwrap();
    let expected = b"export const loaded = true;\n";
    assert_eq!(r.source_code, expected);
    assert_eq!(&(r.media_type), &msg::MediaType::JavaScript);
    let (_, headers) = fetcher.http_cache.get(&module_url).unwrap();
    assert_eq!(headers.get("content-type").unwrap(), "text/javascript");

    // Modify .headers.json
    let mut metadata =
      crate::http_cache::Metadata::read(&cache_filename).unwrap();
    metadata.headers = HashMap::new();
    metadata
      .headers
      .insert("content-type".to_string(), "text/typescript".to_string());
    metadata.write(&cache_filename).unwrap();

    let result2 = fetcher
      .get_source_file(
        &module_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result2.is_ok());
    let r2 = result2.unwrap();
    let expected2 = b"export const loaded = true;\n";
    assert_eq!(r2.source_code, expected2);
    // If get_source_file does not call remote, this should be TypeScript
    // as we modified before! (we do not overwrite .headers.json due to no http
    // fetch)
    assert_eq!(&(r2.media_type), &msg::MediaType::TypeScript);
    let metadata = crate::http_cache::Metadata::read(&cache_filename).unwrap();
    assert_eq!(
      metadata.headers.get("content-type").unwrap(),
      "text/typescript"
    );

    // let's create fresh instance of DenoDir (simulating another fresh Deno
    // process) and don't use cache
    let fetcher = setup_file_fetcher(temp_dir.path());
    let result3 = fetcher
      .get_source_file(
        &module_url_1,
        false,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result3.is_ok());
    let r3 = result3.unwrap();
    let expected3 = b"export const loaded = true;\n";
    assert_eq!(r3.source_code, expected3);
    // Now the old .headers.json file should be overwritten back to JavaScript!
    // (due to http fetch)
    assert_eq!(&(r3.media_type), &msg::MediaType::JavaScript);
    let (_, headers) = fetcher.http_cache.get(&module_url).unwrap();
    assert_eq!(headers.get("content-type").unwrap(), "text/javascript");

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_code_multiple_downloads_of_same_file() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let specifier = ModuleSpecifier::resolve_url(
      "http://localhost:4545/cli/tests/subdir/mismatch_ext.ts",
    )
    .unwrap();
    let cache_filename =
      fetcher.http_cache.get_cache_filename(&specifier.as_url());

    // first download
    let r = fetcher
      .fetch_source_file(&specifier, None, Permissions::allow_all())
      .await;
    assert!(r.is_ok());

    let headers_file_name =
      crate::http_cache::Metadata::filename(&cache_filename);
    let result = fs::File::open(&headers_file_name);
    assert!(result.is_ok());
    let headers_file = result.unwrap();
    // save modified timestamp for headers file
    let headers_file_metadata = headers_file.metadata().unwrap();
    let headers_file_modified = headers_file_metadata.modified().unwrap();

    // download file again, it should use already fetched file even though
    // `use_disk_cache` is set to false, this can be verified using source
    // header file creation timestamp (should be the same as after first
    // download)
    let r = fetcher
      .fetch_source_file(&specifier, None, Permissions::allow_all())
      .await;
    assert!(r.is_ok());

    let result = fs::File::open(&headers_file_name);
    assert!(result.is_ok());
    let headers_file_2 = result.unwrap();
    // save modified timestamp for headers file
    let headers_file_metadata_2 = headers_file_2.metadata().unwrap();
    let headers_file_modified_2 = headers_file_metadata_2.modified().unwrap();

    assert_eq!(headers_file_modified, headers_file_modified_2);
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_code_3() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();

    let redirect_module_url = Url::parse(
      "http://localhost:4546/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();
    let redirect_source_filepath =
      fetcher.http_cache.get_cache_filename(&redirect_module_url);
    let redirect_source_filename =
      redirect_source_filepath.to_str().unwrap().to_string();
    let target_module_url = Url::parse(
      "http://localhost:4545/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();
    let redirect_target_filepath =
      fetcher.http_cache.get_cache_filename(&target_module_url);
    let redirect_target_filename =
      redirect_target_filepath.to_str().unwrap().to_string();

    // Test basic follow and headers recording
    let result = fetcher
      .get_source_file(
        &redirect_module_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let mod_meta = result.unwrap();
    // File that requires redirection should be empty file.
    assert_eq!(fs::read_to_string(&redirect_source_filename).unwrap(), "");
    let (_, headers) = fetcher.http_cache.get(&redirect_module_url).unwrap();
    assert_eq!(
      headers.get("location").unwrap(),
      "http://localhost:4545/cli/tests/subdir/redirects/redirect1.js"
    );
    // The target of redirection is downloaded instead.
    assert_eq!(
      fs::read_to_string(&redirect_target_filename).unwrap(),
      "export const redirect = 1;\n"
    );
    let (_, headers) = fetcher.http_cache.get(&target_module_url).unwrap();
    assert!(headers.get("location").is_none());
    // Examine the meta result.
    assert_eq!(mod_meta.url, target_module_url);

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_code_4() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let double_redirect_url = Url::parse(
      "http://localhost:4548/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();
    let double_redirect_path =
      fetcher.http_cache.get_cache_filename(&double_redirect_url);

    let redirect_url = Url::parse(
      "http://localhost:4546/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();
    let redirect_path = fetcher.http_cache.get_cache_filename(&redirect_url);

    let target_url = Url::parse(
      "http://localhost:4545/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();
    let target_path = fetcher.http_cache.get_cache_filename(&target_url);

    // Test double redirects and headers recording
    let result = fetcher
      .get_source_file(
        &double_redirect_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let mod_meta = result.unwrap();
    assert_eq!(fs::read_to_string(&double_redirect_path).unwrap(), "");
    assert_eq!(fs::read_to_string(&redirect_path).unwrap(), "");

    let (_, headers) = fetcher.http_cache.get(&double_redirect_url).unwrap();
    assert_eq!(headers.get("location").unwrap(), &redirect_url.to_string());

    let (_, headers) = fetcher.http_cache.get(&redirect_url).unwrap();
    assert_eq!(headers.get("location").unwrap(), &target_url.to_string());

    // The target of redirection is downloaded instead.
    assert_eq!(
      fs::read_to_string(&target_path).unwrap(),
      "export const redirect = 1;\n"
    );
    let (_, headers) = fetcher.http_cache.get(&target_url).unwrap();
    assert!(headers.get("location").is_none());

    // Examine the meta result.
    assert_eq!(mod_meta.url, target_url);

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_code_5() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();

    let double_redirect_url = Url::parse(
      "http://localhost:4548/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();

    let redirect_url = Url::parse(
      "http://localhost:4546/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();

    let target_path = fetcher.http_cache.get_cache_filename(&redirect_url);
    let target_path_ = target_path.clone();

    // Test that redirect target is not downloaded twice for different redirect source.
    let result = fetcher
      .get_source_file(
        &double_redirect_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let result = fs::File::open(&target_path);
    assert!(result.is_ok());
    let file = result.unwrap();
    // save modified timestamp for headers file of redirect target
    let file_metadata = file.metadata().unwrap();
    let file_modified = file_metadata.modified().unwrap();

    // When another file is fetched that also point to redirect target, then
    // redirect target shouldn't be downloaded again. It can be verified
    // using source header file creation timestamp (should be the same as
    // after first `get_source_file`)
    let result = fetcher
      .get_source_file(
        &redirect_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let result = fs::File::open(&target_path_);
    assert!(result.is_ok());
    let file_2 = result.unwrap();
    // save modified timestamp for headers file
    let file_metadata_2 = file_2.metadata().unwrap();
    let file_modified_2 = file_metadata_2.modified().unwrap();

    assert_eq!(file_modified, file_modified_2);

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_code_6() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let double_redirect_url = Url::parse(
      "http://localhost:4548/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();

    // Test that redirections can be limited
    let result = fetcher
      .fetch_remote_source(
        &double_redirect_url,
        false,
        false,
        2,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());

    let result = fetcher
      .fetch_remote_source(
        &double_redirect_url,
        false,
        false,
        1,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_err());

    // Test that redirections in cached files are limited as well
    let result = fetcher.fetch_cached_remote_source(&double_redirect_url, 2);
    assert!(result.is_ok());

    let result = fetcher.fetch_cached_remote_source(&double_redirect_url, 1);
    assert!(result.is_err());

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_code_7() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();

    // Testing redirect with Location set to absolute url.
    let redirect_module_url = Url::parse(
      "http://localhost:4550/REDIRECT/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();
    let redirect_source_filepath =
      fetcher.http_cache.get_cache_filename(&redirect_module_url);
    let redirect_source_filename =
      redirect_source_filepath.to_str().unwrap().to_string();
    let target_module_url = Url::parse(
      "http://localhost:4550/cli/tests/subdir/redirects/redirect1.js",
    )
    .unwrap();
    let redirect_target_filepath =
      fetcher.http_cache.get_cache_filename(&target_module_url);
    let redirect_target_filename =
      redirect_target_filepath.to_str().unwrap().to_string();

    // Test basic follow and headers recording
    let result = fetcher
      .get_source_file(
        &redirect_module_url,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let mod_meta = result.unwrap();
    // File that requires redirection should be empty file.
    assert_eq!(fs::read_to_string(&redirect_source_filename).unwrap(), "");
    let (_, headers) = fetcher.http_cache.get(&redirect_module_url).unwrap();
    assert_eq!(
      headers.get("location").unwrap(),
      "/cli/tests/subdir/redirects/redirect1.js"
    );
    // The target of redirection is downloaded instead.
    assert_eq!(
      fs::read_to_string(&redirect_target_filename).unwrap(),
      "export const redirect = 1;\n"
    );
    let (_, headers) = fetcher.http_cache.get(&target_module_url).unwrap();
    assert!(headers.get("location").is_none());
    // Examine the meta result.
    assert_eq!(mod_meta.url, target_module_url);

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_no_remote() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let module_url =
      Url::parse("http://localhost:4545/cli/tests/002_hello.ts").unwrap();
    // Remote modules are not allowed
    let result = fetcher
      .get_source_file(
        &module_url,
        true,
        true,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_err());
    // FIXME(bartlomieju):
    // let err = result.err().unwrap();
    // assert_eq!(err.kind(), ErrorKind::NotFound);

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_get_source_cached_only() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let fetcher_1 = fetcher.clone();
    let fetcher_2 = fetcher.clone();
    let module_url =
      Url::parse("http://localhost:4545/cli/tests/002_hello.ts").unwrap();
    let module_url_1 = module_url.clone();
    let module_url_2 = module_url.clone();

    // file hasn't been cached before
    let result = fetcher
      .get_source_file(
        &module_url,
        true,
        false,
        true,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_err());
    // FIXME(bartlomieju):
    // let err = result.err().unwrap();
    // assert_eq!(err.kind(), ErrorKind::NotFound);

    // download and cache file
    let result = fetcher_1
      .get_source_file(
        &module_url_1,
        true,
        false,
        false,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    // module is already cached, should be ok even with `cached_only`
    let result = fetcher_2
      .get_source_file(
        &module_url_2,
        true,
        false,
        true,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_fetch_source_0() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let module_url =
      Url::parse("http://127.0.0.1:4545/cli/tests/subdir/mt_video_mp2t.t3.ts")
        .unwrap();
    let result = fetcher
      .fetch_remote_source(
        &module_url,
        false,
        false,
        10,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let r = result.unwrap();
    assert_eq!(r.source_code, b"export const loaded = true;\n");
    assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);

    // Modify .metadata.json, make sure read from local
    let cache_filename = fetcher.http_cache.get_cache_filename(&module_url);
    let mut metadata =
      crate::http_cache::Metadata::read(&cache_filename).unwrap();
    metadata.headers = HashMap::new();
    metadata
      .headers
      .insert("content-type".to_string(), "text/javascript".to_string());
    metadata.write(&cache_filename).unwrap();

    let result2 = fetcher.fetch_cached_remote_source(&module_url, 1);
    assert!(result2.is_ok());
    let r2 = result2.unwrap().unwrap();
    assert_eq!(r2.source_code, b"export const loaded = true;\n");
    // Not MediaType::TypeScript due to .headers.json modification
    assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_fetch_source_2() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let fetcher_1 = fetcher.clone();
    let fetcher_2 = fetcher.clone();
    let module_url =
      Url::parse("http://localhost:4545/cli/tests/subdir/no_ext").unwrap();
    let module_url_2 =
      Url::parse("http://localhost:4545/cli/tests/subdir/mismatch_ext.ts")
        .unwrap();
    let module_url_2_ = module_url_2.clone();
    let module_url_3 =
      Url::parse("http://localhost:4545/cli/tests/subdir/unknown_ext.deno")
        .unwrap();
    let module_url_3_ = module_url_3.clone();

    let result = fetcher
      .fetch_remote_source(
        &module_url,
        false,
        false,
        10,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let r = result.unwrap();
    assert_eq!(r.source_code, b"export const loaded = true;\n");
    assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
    let (_, headers) = fetcher.http_cache.get(&module_url).unwrap();
    assert_eq!(headers.get("content-type").unwrap(), "text/typescript");
    let result = fetcher_1
      .fetch_remote_source(
        &module_url_2,
        false,
        false,
        10,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let r2 = result.unwrap();
    assert_eq!(r2.source_code, b"export const loaded = true;\n");
    assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
    let (_, headers) = fetcher.http_cache.get(&module_url_2_).unwrap();
    assert_eq!(headers.get("content-type").unwrap(), "text/javascript");

    // test unknown extension
    let result = fetcher_2
      .fetch_remote_source(
        &module_url_3,
        false,
        false,
        10,
        &Permissions::allow_all(),
      )
      .await;
    assert!(result.is_ok());
    let r3 = result.unwrap();
    assert_eq!(r3.source_code, b"export const loaded = true;\n");
    assert_eq!(&(r3.media_type), &msg::MediaType::TypeScript);
    let (_, headers) = fetcher.http_cache.get(&module_url_3_).unwrap();
    assert_eq!(headers.get("content-type").unwrap(), "text/typescript");

    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_fetch_source_file() {
    let (_temp_dir, fetcher) = test_setup();

    // Test failure case.
    let specifier =
      ModuleSpecifier::resolve_url(file_url!("/baddir/hello.ts")).unwrap();
    let r = fetcher
      .fetch_source_file(&specifier, None, Permissions::allow_all())
      .await;
    assert!(r.is_err());

    let p =
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("js/main.ts");
    let specifier =
      ModuleSpecifier::resolve_url_or_path(p.to_str().unwrap()).unwrap();
    let r = fetcher
      .fetch_source_file(&specifier, None, Permissions::allow_all())
      .await;
    assert!(r.is_ok());
  }

  #[tokio::test]
  async fn test_fetch_source_file_1() {
    /*recompile ts file*/
    let (_temp_dir, fetcher) = test_setup();

    // Test failure case.
    let specifier =
      ModuleSpecifier::resolve_url(file_url!("/baddir/hello.ts")).unwrap();
    let r = fetcher
      .fetch_source_file(&specifier, None, Permissions::allow_all())
      .await;
    assert!(r.is_err());

    let p =
      std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("js/main.ts");
    let specifier =
      ModuleSpecifier::resolve_url_or_path(p.to_str().unwrap()).unwrap();
    let r = fetcher
      .fetch_source_file(&specifier, None, Permissions::allow_all())
      .await;
    assert!(r.is_ok());
  }

  #[tokio::test]
  async fn test_fetch_source_file_2() {
    /*recompile ts file*/
    let (_temp_dir, fetcher) = test_setup();

    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("tests/001_hello.js");
    let specifier =
      ModuleSpecifier::resolve_url_or_path(p.to_str().unwrap()).unwrap();
    let r = fetcher
      .fetch_source_file(&specifier, None, Permissions::allow_all())
      .await;
    assert!(r.is_ok());
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
      assert!(SourceFileFetcher::check_if_supported_scheme(&url).is_err());
    }
  }

  #[test]
  fn test_map_file_extension() {
    assert_eq!(
      map_file_extension(Path::new("foo/bar.ts")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.tsx")),
      msg::MediaType::TSX
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
      map_file_extension(Path::new("foo/bar.jsx")),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.json")),
      msg::MediaType::Json
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.wasm")),
      msg::MediaType::Wasm
    );
    assert_eq!(
      map_file_extension(Path::new("foo/bar.cjs")),
      msg::MediaType::JavaScript
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
  fn test_map_content_type_extension_only() {
    // Extension only
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), None),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.tsx"), None),
      msg::MediaType::TSX
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
      map_content_type(Path::new("foo/bar.txt"), None),
      msg::MediaType::Unknown
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.jsx"), None),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.json"), None),
      msg::MediaType::Json
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.wasm"), None),
      msg::MediaType::Wasm
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.cjs"), None),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), None),
      msg::MediaType::Unknown
    );
  }

  #[test]
  fn test_map_content_type_media_type_with_no_extension() {
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
      map_content_type(Path::new("foo/bar"), Some("application/node")),
      msg::MediaType::JavaScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar"), Some("text/json")),
      msg::MediaType::Json
    );
  }

  #[test]
  fn test_map_file_extension_media_type_with_extension() {
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), Some("text/plain")),
      msg::MediaType::TypeScript
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.ts"), Some("foo/bar")),
      msg::MediaType::Unknown
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.tsx"),
        Some("application/typescript"),
      ),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.tsx"),
        Some("application/javascript"),
      ),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.tsx"),
        Some("application/x-typescript"),
      ),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.tsx"),
        Some("video/vnd.dlna.mpeg-tts"),
      ),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.tsx"), Some("video/mp2t")),
      msg::MediaType::TSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.jsx"),
        Some("application/javascript"),
      ),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.jsx"),
        Some("application/x-typescript"),
      ),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.jsx"),
        Some("application/ecmascript"),
      ),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(Path::new("foo/bar.jsx"), Some("text/ecmascript")),
      msg::MediaType::JSX
    );
    assert_eq!(
      map_content_type(
        Path::new("foo/bar.jsx"),
        Some("application/x-javascript"),
      ),
      msg::MediaType::JSX
    );
  }

  #[test]
  fn test_filter_shebang() {
    assert_eq!(filter_shebang(b"#!"[..].to_owned()), b"");
    assert_eq!(filter_shebang(b"#!\n\n"[..].to_owned()), b"\n\n");
    let code = b"#!/usr/bin/env deno\nconsole.log('hello');\n"[..].to_owned();
    assert_eq!(filter_shebang(code), b"\nconsole.log('hello');\n");
  }

  #[tokio::test]
  async fn test_fetch_with_etag() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let module_url =
      Url::parse("http://127.0.0.1:4545/etag_script.ts").unwrap();

    let source = fetcher
      .fetch_remote_source(
        &module_url,
        false,
        false,
        1,
        &Permissions::allow_all(),
      )
      .await;
    assert!(source.is_ok());
    let source = source.unwrap();
    assert_eq!(source.source_code, b"console.log('etag')");
    assert_eq!(&(source.media_type), &msg::MediaType::TypeScript);

    let (_, headers) = fetcher.http_cache.get(&module_url).unwrap();
    assert_eq!(headers.get("etag").unwrap(), "33a64df551425fcc55e");

    let metadata_path = crate::http_cache::Metadata::filename(
      &fetcher.http_cache.get_cache_filename(&module_url),
    );

    let modified1 = metadata_path.metadata().unwrap().modified().unwrap();

    // Forcibly change the contents of the cache file and request
    // it again with the cache parameters turned off.
    // If the fetched content changes, the cached content is used.
    let file_name = fetcher.http_cache.get_cache_filename(&module_url);
    let _ = fs::write(&file_name, "changed content");
    let cached_source = fetcher
      .fetch_remote_source(
        &module_url,
        false,
        false,
        1,
        &Permissions::allow_all(),
      )
      .await
      .unwrap();
    assert_eq!(cached_source.source_code, b"changed content");

    let modified2 = metadata_path.metadata().unwrap().modified().unwrap();

    // Assert that the file has not been modified
    assert_eq!(modified1, modified2);

    drop(http_server_guard);
  }

  #[test]
  fn test_get_types_url_1() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = b"console.log(\"foo\");".to_owned();
    let result = get_types_url(&module_url, &source_code, None);
    assert_eq!(result, None);
  }

  #[test]
  fn test_get_types_url_2() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = r#"/// <reference types="./mod.d.ts" />
    console.log("foo");"#
      .as_bytes()
      .to_owned();
    let result = get_types_url(&module_url, &source_code, None);
    assert_eq!(
      result,
      Some(Url::parse("https://example.com/mod.d.ts").unwrap())
    );
  }

  #[test]
  fn test_get_types_url_3() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = r#"/// <reference types="https://deno.land/mod.d.ts" />
    console.log("foo");"#
      .as_bytes()
      .to_owned();
    let result = get_types_url(&module_url, &source_code, None);
    assert_eq!(
      result,
      Some(Url::parse("https://deno.land/mod.d.ts").unwrap())
    );
  }

  #[test]
  fn test_get_types_url_4() {
    let module_url = Url::parse("file:///foo/bar/baz.js").unwrap();
    let source_code = r#"/// <reference types="../qat/baz.d.ts" />
    console.log("foo");"#
      .as_bytes()
      .to_owned();
    let result = get_types_url(&module_url, &source_code, None);
    assert_eq!(
      result,
      Some(Url::parse("file:///foo/qat/baz.d.ts").unwrap())
    );
  }

  #[test]
  fn test_get_types_url_5() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = b"console.log(\"foo\");".to_owned();
    let result = get_types_url(&module_url, &source_code, Some("./mod.d.ts"));
    assert_eq!(
      result,
      Some(Url::parse("https://example.com/mod.d.ts").unwrap())
    );
  }

  #[test]
  fn test_get_types_url_6() {
    let module_url = Url::parse("https://example.com/mod.js").unwrap();
    let source_code = r#"/// <reference types="./mod.d.ts" />
    console.log("foo");"#
      .as_bytes()
      .to_owned();
    let result = get_types_url(
      &module_url,
      &source_code,
      Some("https://deno.land/mod.d.ts"),
    );
    assert_eq!(
      result,
      Some(Url::parse("https://deno.land/mod.d.ts").unwrap())
    );
  }

  #[tokio::test]
  async fn test_fetch_with_types_header() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let module_url =
      Url::parse("http://127.0.0.1:4545/xTypeScriptTypes.js").unwrap();
    let source = fetcher
      .fetch_remote_source(
        &module_url,
        false,
        false,
        1,
        &Permissions::allow_all(),
      )
      .await;
    assert!(source.is_ok());
    let source = source.unwrap();
    assert_eq!(source.source_code, b"export const foo = 'foo';");
    assert_eq!(&(source.media_type), &msg::MediaType::JavaScript);
    assert_eq!(
      source.types_url,
      Some(Url::parse("http://127.0.0.1:4545/xTypeScriptTypes.d.ts").unwrap())
    );
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn test_fetch_with_types_reference() {
    let http_server_guard = crate::test_util::http_server();
    let (_temp_dir, fetcher) = test_setup();
    let module_url =
      Url::parse("http://127.0.0.1:4545/referenceTypes.js").unwrap();
    let source = fetcher
      .fetch_remote_source(
        &module_url,
        false,
        false,
        1,
        &Permissions::allow_all(),
      )
      .await;
    assert!(source.is_ok());
    let source = source.unwrap();
    assert_eq!(&(source.media_type), &msg::MediaType::JavaScript);
    assert_eq!(
      source.types_url,
      Some(Url::parse("http://127.0.0.1:4545/xTypeScriptTypes.d.ts").unwrap())
    );
    drop(http_server_guard);
  }
}
