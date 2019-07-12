// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#![allow(dead_code)]
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::deno_error::GetErrorKind;
use crate::fs as deno_fs;
use crate::http_util;
use crate::msg;
use crate::progress::Progress;
use crate::tokio_util;
use deno::ErrBox;
use deno::ModuleSpecifier;
use dirs;
use futures::future::{loop_fn, Either, Loop};
use futures::Future;
use http;
use serde_json;
use std;
use std::collections::HashSet;
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

#[derive(Debug, Clone)]
pub struct SourceFile {
  pub url: Url,
  pub redirect_source_url: Option<Url>,
  pub filename: PathBuf,
  pub media_type: msg::MediaType,
  pub source_code: Vec<u8>,
  // TODO: those fields shouldn't be on this struct, they are orthogonal to source file
  // and only useful in compiled modules
  pub maybe_source_map_filename: Option<PathBuf>,
  pub maybe_source_map: Option<Vec<u8>>,
}

impl SourceFile {
  // TODO: this method should be implemented on CompiledSourceFile trait
  pub fn js_source(&self) -> String {
    if self.media_type == msg::MediaType::TypeScript {
      panic!("TypeScript module has no JS source, did you forget to run it through compiler?");
    }

    // TODO: this should be done by compiler and JS module should be returned
    if self.media_type == msg::MediaType::Json {
      return format!(
        "export default {};",
        str::from_utf8(&self.source_code).unwrap()
      );
    }

    // it's either JS or Unknown media type
    str::from_utf8(&self.source_code).unwrap().to_string()
  }
}

pub type SourceFileFuture =
  dyn Future<Item = SourceFile, Error = ErrBox> + Send;

pub trait SourceFileFetcher {
  fn fetch_source_file_async(
    self: &Self,
    specifier: &ModuleSpecifier,
    use_cache: bool,
    no_fetch: bool,
  ) -> Box<SourceFileFuture>;

  /// Synchronous version of fetch_source_file_async
  /// Required for TS compiler.
  fn fetch_source_file(
    self: &Self,
    specifier: &ModuleSpecifier,
    use_cache: bool,
    no_fetch: bool,
  ) -> Result<SourceFile, ErrBox>;
}

// TODO: this list should be implemented on SourceFileFetcher trait
const SUPPORTED_URL_SCHEMES: [&str; 3] = ["http", "https", "file"];

fn normalize_path(path: &Path) -> PathBuf {
  let s = String::from(path.to_str().unwrap());
  let normalized_string = if cfg!(windows) {
    // TODO This isn't correct. Probbly should iterate over components.
    s.replace("\\", "/")
  } else {
    s
  };

  PathBuf::from(normalized_string)
}

#[derive(Clone, Default)]
pub struct DownloadCache(Arc<Mutex<HashSet<String>>>);

impl DownloadCache {
  pub fn mark(&self, module_id: &str) {
    let mut c = self.0.lock().unwrap();
    c.insert(module_id.to_string());
  }

  pub fn has(&self, module_id: &str) -> bool {
    let c = self.0.lock().unwrap();
    c.contains(module_id)
  }
}

// TODO: this is a structure that is supposed to be moved to //core/cache.rs
#[derive(Clone)]
pub struct DiskCache {
  location: PathBuf,
}

impl DiskCache {
  pub fn new(location: &Path) -> Self {
    // TODO: ensure that 'location' is a directory
    Self {
      location: location.to_owned(),
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
    // Write file and create .headers.json for the file.
    deno_fs::write_file(&path, data, 0o666)
  }
}

#[derive(Clone)]
pub struct DenoDir {
  // Example: /Users/rld/.deno/
  pub root: PathBuf,
  // In the Go code this was called SrcDir.
  // This is where we cache http resources. Example:
  // /Users/rld/.deno/deps/github.com/ry/blah.js
  pub gen: PathBuf,
  pub gen_cache: DiskCache,
  // In the Go code this was called CacheDir.
  // This is where we cache compilation outputs. Example:
  // /Users/rld/.deno/gen/f39a473452321cacd7c346a870efb0e3e1264b43.js
  pub deps: PathBuf,
  pub deps_cache: DiskCache,
  // This splits to http and https deps
  pub deps_http: PathBuf,
  pub deps_https: PathBuf,

  pub progress: Progress,

  /// Set of all URLs that have been fetched in this run. This is a hacky way to work
  /// around the fact that --reload will force multiple downloads of the same
  /// module.
  download_cache: DownloadCache,
}

impl DenoDir {
  // Must be called before using any function from this module.
  // https://github.com/denoland/deno/blob/golang/deno_dir.go#L99-L111
  pub fn new(
    custom_root: Option<PathBuf>,
    progress: Progress,
  ) -> std::io::Result<Self> {
    // Only setup once.
    let home_dir = dirs::home_dir().expect("Could not get home directory.");
    let fallback = home_dir.join(".deno");
    // We use the OS cache dir because all files deno writes are cache files
    // Once that changes we need to start using different roots if DENO_DIR
    // is not set, and keep a single one if it is.
    let default = dirs::cache_dir()
      .map(|d| d.join("deno"))
      .unwrap_or(fallback);

    let root: PathBuf = custom_root.unwrap_or(default);
    let gen = root.as_path().join("gen");
    let deps = root.as_path().join("deps");
    let deps_http = deps.join("http");
    let deps_https = deps.join("https");

    let deno_dir = Self {
      root,
      gen_cache: DiskCache::new(&gen),
      gen,
      deps_cache: DiskCache::new(&deps),
      deps,
      deps_http,
      deps_https,
      progress,
      download_cache: DownloadCache::default(),
    };

    // TODO Lazily create these directories.
    deno_fs::mkdir(deno_dir.gen.as_ref(), 0o755, true)?;
    deno_fs::mkdir(deno_dir.deps.as_ref(), 0o755, true)?;
    deno_fs::mkdir(deno_dir.deps_http.as_ref(), 0o755, true)?;
    deno_fs::mkdir(deno_dir.deps_https.as_ref(), 0o755, true)?;

    debug!("root {}", deno_dir.root.display());
    debug!("gen {}", deno_dir.gen.display());
    debug!("deps {}", deno_dir.deps.display());
    debug!("deps_http {}", deno_dir.deps_http.display());
    debug!("deps_https {}", deno_dir.deps_https.display());

    Ok(deno_dir)
  }

  /// This method returns local file path for given module url that is used
  /// internally by DenoDir to reference module.
  ///
  /// For specifiers starting with `file://` returns the input.
  ///
  /// For specifier starting with `http://` and `https://` it returns
  /// path to DenoDir dependency directory.
  pub fn url_to_deps_path(self: &Self, url: &Url) -> Result<PathBuf, ErrBox> {
    let filename = match url.scheme() {
      "file" => url.to_file_path().unwrap(),
      "https" => self.deps.join(get_cache_filename(&url)),
      "http" => self.deps.join(get_cache_filename(&url)),
      scheme => {
        return Err(
          DenoError::new(
            ErrorKind::UnsupportedFetchScheme,
            format!("Unsupported scheme \"{}\" for module \"{}\". Supported schemes: {:#?}", scheme, url, SUPPORTED_URL_SCHEMES),
          ).into()
        );
      }
    };

    debug!("deps filename: {:?}", filename);
    Ok(normalize_path(&filename))
  }
}

impl SourceFileFetcher for DenoDir {
  fn fetch_source_file_async(
    self: &Self,
    specifier: &ModuleSpecifier,
    use_cache: bool,
    no_fetch: bool,
  ) -> Box<SourceFileFuture> {
    let module_url = specifier.as_url().to_owned();
    debug!("fetch_source_file. specifier {} ", &module_url);

    let result = self.url_to_deps_path(&module_url);
    if let Err(err) = result {
      return Box::new(futures::future::err(err));
    }
    let deps_filepath = result.unwrap();

    let fut = get_source_code_async(
      self,
      &module_url,
      deps_filepath.clone(),
      use_cache,
      no_fetch,
    ).then(move |result| {
      let mut out = result.map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
          // For NotFound, change the message to something better.
          DenoError::new(
            ErrorKind::NotFound,
            format!("Cannot resolve module \"{}\"", module_url.to_string()),
          ).into()
        } else {
          err
        }
      })?;

      if out.source_code.starts_with(b"#!") {
        out.source_code = filter_shebang(out.source_code);
      }

      Ok(out)
    });

    Box::new(fut)
  }

  /// Synchronous version of fetch_source_file_async
  /// This function is deprecated.
  fn fetch_source_file(
    self: &Self,
    specifier: &ModuleSpecifier,
    use_cache: bool,
    no_fetch: bool,
  ) -> Result<SourceFile, ErrBox> {
    tokio_util::block_on(
      self.fetch_source_file_async(specifier, use_cache, no_fetch),
    )
  }
}

/// This fetches source code, locally or remotely.
/// module_name is the URL specifying the module.
/// filename is the local path to the module (if remote, it is in the cache
/// folder, and potentially does not exist yet)
///
/// It *does not* fill the compiled JS nor source map portions of
/// SourceFile. This is the only difference between this function and
/// fetch_source_file_async(). TODO(ry) change return type to reflect this
/// fact.
///
/// If this is a remote module, and it has not yet been cached, the resulting
/// download will be written to "filename". This happens no matter the value of
/// use_cache.
fn get_source_code_async(
  deno_dir: &DenoDir,
  module_url: &Url,
  filepath: PathBuf,
  use_cache: bool,
  no_fetch: bool,
) -> impl Future<Item = SourceFile, Error = ErrBox> {
  let filename = filepath.to_str().unwrap().to_string();
  let url_scheme = module_url.scheme();
  let is_local_file = url_scheme == "file";

  // Local files are always fetched from disk bypassing cache.
  if is_local_file {
    match fetch_local_source(deno_dir, &module_url, &filepath, None) {
      Ok(Some(source_file)) => {
        return Either::A(futures::future::ok(source_file));
      },
      Ok(None) => {
        return Either::A(futures::future::err(
          std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("cannot find local file '{}'", filename),
          ).into(),
        ));
      }
      Err(err) => {
        return Either::A(futures::future::err(err));
      }
    }
  }

  // We're dealing with remote file, first try local cache
  if use_cache {
    match fetch_local_source(deno_dir, &module_url, &filepath, None) {
      Ok(Some(source_file)) => {
        return Either::A(futures::future::ok(source_file));
      },
      Ok(None) => {
        // there's no cached version
      }
      Err(err) => {
        return Either::A(futures::future::err(err));
      }
    }
  }

  // If remote file wasn't found check if we can fetch it
  if no_fetch {
    // We can't fetch remote file - bail out
    return Either::A(futures::future::err(
      std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("cannot find remote file '{}' in cache", filename),
      ).into(),
    ));
  }

  // Fetch remote file and cache on-disk for subsequent access
  // not cached/local, try remote.
  Either::B(
    fetch_remote_source_async(deno_dir, &module_url, &filepath).and_then(
      move |maybe_remote_source| match maybe_remote_source {
        Some(output) => Ok(output),
        None => Err(
          std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("cannot find remote file '{}'", filename),
          ).into(),
        ),
      },
    ),
  )
}

// TODO: method of DiskCache?
pub fn get_cache_filename(url: &Url) -> PathBuf {
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

/// Save source code and related headers for given module
fn save_module_code_and_headers(
  filepath: PathBuf,
  module_url: &Url,
  source: &str,
  maybe_content_type: Option<String>,
  maybe_initial_filepath: Option<PathBuf>,
) -> Result<(), ErrBox> {
  match filepath.parent() {
    Some(ref parent) => fs::create_dir_all(parent),
    None => Ok(()),
  }?;
  // Write file and create .headers.json for the file.
  deno_fs::write_file(&filepath, &source, 0o666)?;
  {
    save_source_code_headers(&filepath, maybe_content_type.clone(), None);
  }
  // Check if this file is downloaded due to some old redirect request.
  if maybe_initial_filepath.is_some() {
    // If yes, record down the headers for redirect.
    // Also create its containing folder.
    match filepath.parent() {
      Some(ref parent) => fs::create_dir_all(parent),
      None => Ok(()),
    }?;
    {
      save_source_code_headers(
        &maybe_initial_filepath.unwrap(),
        maybe_content_type.clone(),
        Some(module_url.to_string()),
      );
    }
  }

  Ok(())
}

fn url_into_uri(url: &url::Url) -> http::uri::Uri {
  http::uri::Uri::from_str(&url.to_string())
    .expect("url::Url should be parseable as http::uri::Uri")
}

/// Asynchronously fetch remote source file specified by the URL `module_name`
/// and write it to disk at `filename`.
fn fetch_remote_source_async(
  deno_dir: &DenoDir,
  module_url: &Url,
  filepath: &Path,
) -> impl Future<Item = Option<SourceFile>, Error = ErrBox> {
  use crate::http_util::FetchOnceResult;

  let download_job = deno_dir.progress.add("Download", &module_url.to_string());

  let filepath = filepath.to_owned();

  // We write a special ".headers.json" file into the `.deno/deps` directory along side the
  // cached file, containing just the media type and possible redirect target (both are http headers).
  // If redirect target is present, the file itself if not cached.
  // In future resolutions, we would instead follow this redirect target ("redirect_to").
  loop_fn(
    (
      deno_dir.clone(),
      None,
      None,
      module_url.clone(),
      filepath.clone(),
    ),
    |(
      dir,
      mut maybe_initial_module_url,
      mut maybe_initial_filepath,
      module_url,
      filepath,
    )| {
      let module_uri = url_into_uri(&module_url);
      // Single pass fetch, either yields code or yields redirect.
      http_util::fetch_string_once(module_uri).and_then(
        move |fetch_once_result| {
          match fetch_once_result {
            FetchOnceResult::Redirect(uri) => {
              // If redirects, update module_name and filename for next looped call.
              let new_module_url = Url::parse(&uri.to_string()).expect("http::uri::Uri should be parseable as Url");
              let new_filepath = dir.url_to_deps_path(&new_module_url)?;

              if maybe_initial_module_url.is_none() {
                maybe_initial_module_url = Some(module_url);
                maybe_initial_filepath = Some(filepath.clone());
              }

              // Not yet completed. Follow the redirect and loop.
              Ok(Loop::Continue((
                dir,
                maybe_initial_module_url,
                maybe_initial_filepath,
                new_module_url,
                new_filepath,
              )))
            }
            FetchOnceResult::Code(source, maybe_content_type) => {
              // TODO: this should be done using `DiskCache` API
              // We land on the code.
              save_module_code_and_headers(
                filepath.clone(),
                &module_url,
                &source,
                maybe_content_type.clone(),
                maybe_initial_filepath,
              )?;

              let media_type = map_content_type(
                &filepath,
                maybe_content_type.as_ref().map(String::as_str),
              );

              let source_file = SourceFile {
                url: module_url,
                redirect_source_url: maybe_initial_module_url,
                filename: filepath.clone(),
                media_type,
                source_code: source.as_bytes().to_owned(),
                maybe_source_map_filename: None,
                maybe_source_map: None,
              };

              Ok(Loop::Break(Some(source_file)))
            }
          }
        },
      )
    },
  )
  .then(move |r| {
    // Explicit drop to keep reference alive until future completes.
    drop(download_job);
    r
  })
}

/// Fetch remote source code.
#[cfg(test)]
fn fetch_remote_source(
  deno_dir: &DenoDir,
  module_url: &Url,
  filepath: &Path,
) -> Result<Option<SourceFile>, ErrBox> {
  tokio_util::block_on(fetch_remote_source_async(
    deno_dir, module_url, filepath,
  ))
}

/// Fetch local or cached source code.
/// This is a recursive operation if source file has redirection.
/// It will keep reading filename.headers.json for information about redirection.
/// module_initial_source_name would be None on first call,
/// and becomes the name of the very first module that initiates the call
/// in subsequent recursions.
/// AKA if redirection occurs, module_initial_source_name is the source path
/// that user provides, and the final module_name is the resolved path
/// after following all redirections.
fn fetch_local_source(
  deno_dir: &DenoDir,
  module_url: &Url,
  filepath: &Path,
  maybe_initial_module_url: Option<Url>,
) -> Result<Option<SourceFile>, ErrBox> {
  let source_code_headers = get_source_code_headers(&filepath);
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
    let real_module_url =
      Url::parse(&redirect_to).expect("Should be valid URL");
    let real_filepath = deno_dir.url_to_deps_path(&real_module_url)?;

    let mut maybe_initial_module_url = maybe_initial_module_url;
    // If this is the first redirect attempt,
    // then maybe_initial_module_url should be None.
    // In that case, use current module name as maybe_initial_module_url.
    if maybe_initial_module_url.is_none() {
      maybe_initial_module_url = Some(module_url.clone());
    }
    // Recurse.
    return fetch_local_source(
      deno_dir,
      &real_module_url,
      &real_filepath,
      maybe_initial_module_url,
    );
  }
  // No redirect needed or end of redirects.
  // We can try read the file
  let source_code = match fs::read(filepath) {
    Err(e) => {
      if e.kind() == std::io::ErrorKind::NotFound {
        return Ok(None);
      } else {
        return Err(e.into());
      }
    }
    Ok(c) => c,
  };
  Ok(Some(SourceFile {
    url: module_url.clone(),
    redirect_source_url: maybe_initial_module_url,
    filename: filepath.to_owned(),
    media_type: map_content_type(
      &filepath,
      source_code_headers.mime_type.as_ref().map(String::as_str),
    ),
    source_code,
    maybe_source_map_filename: None,
    maybe_source_map: None,
  }))
}

#[derive(Debug)]
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

fn source_code_headers_filename(filepath: &Path) -> PathBuf {
  PathBuf::from([filepath.to_str().unwrap(), ".headers.json"].concat())
}

/// Get header metadata associated with a single source code file.
/// NOTICE: chances are that the source code itself is not downloaded due to redirects.
/// In this case, the headers file provides info about where we should go and get
/// the source code that redirect eventually points to (which should be cached).
fn get_source_code_headers(filepath: &Path) -> SourceCodeHeaders {
  let headers_filename = source_code_headers_filename(filepath);
  let hd = Path::new(&headers_filename);
  // .headers.json file might not exists.
  // This is okay for local source.
  let maybe_headers_string = fs::read_to_string(&hd).ok();
  if let Some(headers_string) = maybe_headers_string {
    // TODO(kevinkassimo): consider introduce serde::Deserialize to make things simpler.
    let maybe_headers: serde_json::Result<serde_json::Value> =
      serde_json::from_str(&headers_string);
    if let Ok(headers) = maybe_headers {
      return SourceCodeHeaders {
        mime_type: headers[MIME_TYPE].as_str().map(String::from),
        redirect_to: headers[REDIRECT_TO].as_str().map(String::from),
      };
    }
  }
  SourceCodeHeaders {
    mime_type: None,
    redirect_to: None,
  }
}

/// Save headers related to source filename to {filename}.headers.json file,
/// only when there is actually something necessary to save.
/// For example, if the extension ".js" already mean JS file and we have
/// content type of "text/javascript", then we would not save the mime type.
/// If nothing needs to be saved, the headers file is not created.
fn save_source_code_headers(
  filepath: &Path,
  mime_type: Option<String>,
  redirect_to: Option<String>,
) {
  let headers_filename = source_code_headers_filename(filepath);
  // Remove possibly existing stale .headers.json file.
  // May not exist. DON'T unwrap.
  let _ = std::fs::remove_file(&headers_filename);
  // TODO(kevinkassimo): consider introduce serde::Deserialize to make things simpler.
  // This is super ugly at this moment...
  // Had trouble to make serde_derive work: I'm unable to build proc-macro2.
  let mut value_map = serde_json::map::Map::new();
  if mime_type.is_some() {
    let mime_type_string = mime_type.clone().unwrap();
    let resolved_mime_type =
      { map_content_type(Path::new(""), Some(mime_type_string.as_str())) };
    let ext_based_mime_type = map_file_extension(filepath);
    // Add mime to headers only when content type is different from extension.
    if ext_based_mime_type == msg::MediaType::Unknown
      || resolved_mime_type != ext_based_mime_type
    {
      value_map.insert(MIME_TYPE.to_string(), json!(mime_type_string));
    }
  }
  if redirect_to.is_some() {
    value_map.insert(REDIRECT_TO.to_string(), json!(redirect_to.unwrap()));
  }
  // Only save to file when there is actually data.
  if !value_map.is_empty() {
    let _ = serde_json::to_string(&value_map).map(|s| {
      // It is possible that we need to create file
      // with parent folders not yet created.
      // (Due to .headers.json feature for redirection)
      let hd = PathBuf::from(&headers_filename);
      let _ = match hd.parent() {
        Some(ref parent) => fs::create_dir_all(parent),
        None => Ok(()),
      };
      let _ = deno_fs::write_file(&(hd.as_path()), s, 0o666);
    });
  }
}

// TODO(bartlomieju): this method should be moved, it doesn't belong to deno_dir.rs
//  it's a general utility
pub fn resolve_from_cwd(path: &str) -> Result<(PathBuf, String), ErrBox> {
  let candidate_path = Path::new(path);

  let resolved_path = if candidate_path.is_absolute() {
    candidate_path.to_owned()
  } else {
    let cwd = std::env::current_dir().unwrap();
    cwd.join(path)
  };

  // HACK: `Url::from_directory_path` is used here because it normalizes the path.
  // Joining `/dev/deno/" with "./tests" using `PathBuf` yields `/deno/dev/./tests/`.
  // On the other hand joining `/dev/deno/" with "./tests" using `Url` yields "/dev/deno/tests"
  // - and that's what we want.
  // There exists similar method on `PathBuf` - `PathBuf.canonicalize`, but the problem
  // is `canonicalize` resolves symlinks and we don't want that.
  // We just want o normalize the path...
  let resolved_url = Url::from_file_path(resolved_path)
    .expect("PathBuf should be parseable URL");
  let normalized_path = resolved_url
    .to_file_path()
    .expect("URL from PathBuf should be valid path");

  let path_string = normalized_path.to_str().unwrap().to_string();

  Ok((normalized_path, path_string))
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  /// Synchronous version of get_source_code_async
  fn get_source_code(
    deno_dir: &DenoDir,
    module_url: &Url,
    filepath: PathBuf,
    use_cache: bool,
    no_fetch: bool,
  ) -> Result<SourceFile, ErrBox> {
    tokio_util::block_on(get_source_code_async(
      deno_dir, module_url, filepath, use_cache, no_fetch,
    ))
  }

  fn normalize_to_str(path: &Path) -> String {
    normalize_path(path).to_str().unwrap().to_string()
  }

  fn setup_deno_dir(dir_path: &Path) -> DenoDir {
    DenoDir::new(Some(dir_path.to_path_buf()), Progress::new())
      .expect("setup fail")
  }

  fn test_setup() -> (TempDir, DenoDir) {
    let temp_dir = TempDir::new().expect("tempdir fail");
    let deno_dir = setup_deno_dir(temp_dir.path());
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
  fn test_get_cache_filename() {
    let url = Url::parse("http://example.com:1234/path/to/file.ts").unwrap();
    let cache_file = get_cache_filename(&url);
    assert_eq!(
      cache_file,
      Path::new("http/example.com_PORT1234/path/to/file.ts")
    );

    let url = Url::parse("file:///src/a/b/c/file.ts").unwrap();
    let cache_file = get_cache_filename(&url);
    assert_eq!(cache_file, Path::new("file//src/a/b/c/file.ts"));
  }

  #[test]
  fn test_cache_paths() {
    let file_url = Url::parse("file:///a/b/c/hello.js").unwrap();
    let cache_filename = get_cache_filename(&file_url);
    assert_eq!(
      (
        cache_filename.with_extension("js"),
        cache_filename.with_extension("js.map"),
        cache_filename.with_extension("meta"),
      ),
      (
        PathBuf::from("file/a/b/c/hello.js"),
        PathBuf::from("file/a/b/c/hello.js.map"),
        PathBuf::from("file/a/b/c/hello.meta"),
      )
    );
  }

  #[test]
  fn test_source_code_headers_get_and_save() {
    let (temp_dir, _deno_dir) = test_setup();
    let filepath = temp_dir.into_path().join("f.js");
    let headers_filepath = source_code_headers_filename(&filepath);
    assert_eq!(
      headers_filepath.to_str().unwrap().to_string(),
      [filepath.to_str().unwrap(), ".headers.json"].concat()
    );
    let _ = deno_fs::write_file(headers_filepath.as_path(),
      "{\"mime_type\":\"text/javascript\",\"redirect_to\":\"http://example.com/a.js\"}", 0o666);
    let headers = get_source_code_headers(&filepath);
    assert_eq!(headers.mime_type.clone().unwrap(), "text/javascript");
    assert_eq!(
      headers.redirect_to.clone().unwrap(),
      "http://example.com/a.js"
    );

    save_source_code_headers(
      &filepath,
      Some("text/typescript".to_owned()),
      Some("http://deno.land/a.js".to_owned()),
    );
    let headers2 = get_source_code_headers(&filepath);
    assert_eq!(headers2.mime_type.clone().unwrap(), "text/typescript");
    assert_eq!(
      headers2.redirect_to.clone().unwrap(),
      "http://deno.land/a.js"
    );
  }

  #[test]
  fn test_get_source_code_1() {
    let (temp_dir, deno_dir) = test_setup();
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/mod2.ts").unwrap();
      let filepath = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/mod2.ts");
      let headers_file_name = source_code_headers_filename(&filepath);

      let result =
        get_source_code(&deno_dir, &module_url, filepath.clone(), true, false);
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
      let result2 =
        get_source_code(&deno_dir, &module_url, filepath.clone(), true, false);
      assert!(result2.is_ok());
      let r2 = result2.unwrap();
      assert_eq!(
        r2.source_code,
        "export { printHello } from \"./print_hello.ts\";\n".as_bytes()
      );
      // If get_source_code does not call remote, this should be JavaScript
      // as we modified before! (we do not overwrite .headers.json due to no http fetch)
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        get_source_code_headers(&filepath).mime_type.unwrap(),
        "text/javascript"
      );

      // Modify .headers.json again, but the other way around
      save_source_code_headers(
        &filepath,
        Some("application/json".to_owned()),
        None,
      );
      let result3 =
        get_source_code(&deno_dir, &module_url, filepath.clone(), true, false);
      assert!(result3.is_ok());
      let r3 = result3.unwrap();
      assert_eq!(
        r3.source_code,
        "export { printHello } from \"./print_hello.ts\";\n".as_bytes()
      );
      // If get_source_code does not call remote, this should be JavaScript
      // as we modified before! (we do not overwrite .headers.json due to no http fetch)
      assert_eq!(&(r3.media_type), &msg::MediaType::Json);
      assert!(
        fs::read_to_string(&headers_file_name)
          .unwrap()
          .contains("application/json")
      );

      // let's create fresh instance of DenoDir (simulating another freshh Deno process)
      // and don't use cache
      let deno_dir = setup_deno_dir(temp_dir.path());
      let result4 =
        get_source_code(&deno_dir, &module_url, filepath.clone(), false, false);
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
    let (temp_dir, deno_dir) = test_setup();
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/mismatch_ext.ts")
          .unwrap();
      let filepath = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/mismatch_ext.ts");
      let headers_file_name = source_code_headers_filename(&filepath);

      let result =
        get_source_code(&deno_dir, &module_url, filepath.clone(), true, false);
      assert!(result.is_ok());
      let r = result.unwrap();
      let expected = "export const loaded = true;\n".as_bytes();
      assert_eq!(r.source_code, expected);
      // Mismatch ext with content type, create .headers.json
      assert_eq!(&(r.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        get_source_code_headers(&filepath).mime_type.unwrap(),
        "text/javascript"
      );

      // Modify .headers.json
      save_source_code_headers(
        &filepath,
        Some("text/typescript".to_owned()),
        None,
      );
      let result2 =
        get_source_code(&deno_dir, &module_url, filepath.clone(), true, false);
      assert!(result2.is_ok());
      let r2 = result2.unwrap();
      let expected2 = "export const loaded = true;\n".as_bytes();
      assert_eq!(r2.source_code, expected2);
      // If get_source_code does not call remote, this should be TypeScript
      // as we modified before! (we do not overwrite .headers.json due to no http fetch)
      assert_eq!(&(r2.media_type), &msg::MediaType::TypeScript);
      assert!(fs::read_to_string(&headers_file_name).is_err());

      // let's create fresh instance of DenoDir (simulating another freshh Deno process)
      // and don't use cache
      let deno_dir = setup_deno_dir(temp_dir.path());
      let result3 =
        get_source_code(&deno_dir, &module_url, filepath.clone(), false, false);
      assert!(result3.is_ok());
      let r3 = result3.unwrap();
      let expected3 = "export const loaded = true;\n".as_bytes();
      assert_eq!(r3.source_code, expected3);
      // Now the old .headers.json file should be overwritten back to JavaScript!
      // (due to http fetch)
      assert_eq!(&(r3.media_type), &msg::MediaType::JavaScript);
      assert_eq!(
        get_source_code_headers(&filepath).mime_type.unwrap(),
        "text/javascript"
      );
    });
  }

  #[test]
  fn test_get_source_code_multiple_downloads_of_same_file() {
    let (_temp_dir, deno_dir) = test_setup();
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/mismatch_ext.ts")
          .unwrap();
      let filepath = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/mismatch_ext.ts");
      let headers_file_name = source_code_headers_filename(&filepath);

      // first download
      let result =
        get_source_code(&deno_dir, &module_url, filepath.clone(), false, false);
      assert!(result.is_ok());

      let result = fs::File::open(&headers_file_name);
      assert!(result.is_ok());
      let headers_file = result.unwrap();
      // save modified timestamp for headers file
      let headers_file_metadata = headers_file.metadata().unwrap();
      let headers_file_modified = headers_file_metadata.modified().unwrap();

      // download file again, it should use already fetched file even though `use_cache` is set to
      // false, this can be verified using source header file creation timestamp (should be
      // the same as after first download)
      let result =
        get_source_code(&deno_dir, &module_url, filepath.clone(), false, false);
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
    let (_temp_dir, deno_dir) = test_setup();
    // Test basic follow and headers recording
    tokio_util::init(|| {
      let redirect_module_url =
        Url::parse("http://localhost:4546/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let redirect_source_filepath = deno_dir
        .deps_http
        .join("localhost_PORT4546/tests/subdir/redirects/redirect1.js");
      let redirect_source_filename =
        redirect_source_filepath.to_str().unwrap().to_string();
      let target_module_url =
        Url::parse("http://localhost:4545/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let redirect_target_filepath = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/redirects/redirect1.js");
      let redirect_target_filename =
        redirect_target_filepath.to_str().unwrap().to_string();

      let mod_meta = get_source_code(
        &deno_dir,
        &redirect_module_url,
        redirect_source_filepath.clone(),
        true,
        false,
      ).unwrap();
      // File that requires redirection is not downloaded.
      assert!(fs::read_to_string(&redirect_source_filename).is_err());
      // ... but its .headers.json is created.
      let redirect_source_headers =
        get_source_code_headers(&redirect_source_filepath);
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
        get_source_code_headers(&redirect_target_filepath);
      assert!(redirect_target_headers.redirect_to.is_none());

      // Examine the meta result.
      assert_eq!(mod_meta.url.clone(), target_module_url);
      assert_eq!(
        &mod_meta.redirect_source_url.clone().unwrap(),
        &redirect_module_url
      );
    });
  }

  #[test]
  fn test_get_source_code_4() {
    let (_temp_dir, deno_dir) = test_setup();
    // Test double redirects and headers recording
    tokio_util::init(|| {
      let redirect_module_url =
        Url::parse("http://localhost:4548/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let redirect_source_filepath = deno_dir
        .deps_http
        .join("localhost_PORT4548/tests/subdir/redirects/redirect1.js");
      let redirect_source_filename =
        redirect_source_filepath.to_str().unwrap().to_string();
      let redirect_source_filename_intermediate = normalize_to_str(
        deno_dir
          .deps_http
          .join("localhost_PORT4546/tests/subdir/redirects/redirect1.js")
          .as_ref(),
      );
      let target_module_url =
        Url::parse("http://localhost:4545/tests/subdir/redirects/redirect1.js")
          .unwrap();
      let target_module_name = target_module_url.to_string();
      let redirect_target_filepath = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/redirects/redirect1.js");
      let redirect_target_filename =
        redirect_target_filepath.to_str().unwrap().to_string();

      let mod_meta = get_source_code(
        &deno_dir,
        &redirect_module_url,
        redirect_source_filepath.clone(),
        true,
        false,
      ).unwrap();

      // File that requires redirection is not downloaded.
      assert!(fs::read_to_string(&redirect_source_filename).is_err());
      // ... but its .headers.json is created.
      let redirect_source_headers =
        get_source_code_headers(&redirect_source_filepath);
      assert_eq!(
        redirect_source_headers.redirect_to.unwrap(),
        target_module_name
      );

      // In the intermediate redirection step, file is also not downloaded.
      assert!(
        fs::read_to_string(&redirect_source_filename_intermediate).is_err()
      );

      // The target of redirection is downloaded instead.
      assert_eq!(
        fs::read_to_string(&redirect_target_filename).unwrap(),
        "export const redirect = 1;\n"
      );
      let redirect_target_headers =
        get_source_code_headers(&redirect_target_filepath);
      assert!(redirect_target_headers.redirect_to.is_none());

      // Examine the meta result.
      assert_eq!(mod_meta.url.clone(), target_module_url);
      assert_eq!(
        &mod_meta.redirect_source_url.clone().unwrap(),
        &redirect_module_url
      );
    });
  }

  #[test]
  fn test_get_source_code_no_fetch() {
    let (_temp_dir, deno_dir) = test_setup();
    tokio_util::init(|| {
      let module_url =
        Url::parse("http://localhost:4545/tests/002_hello.ts").unwrap();
      let filepath = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/002_hello.ts");

      // file hasn't been cached before and remote downloads are not allowed
      let result =
        get_source_code(&deno_dir, &module_url, filepath.clone(), true, true);
      assert!(result.is_err());
      let err = result.err().unwrap();
      assert_eq!(err.kind(), ErrorKind::NotFound);

      // download and cache file
      let result =
        get_source_code(&deno_dir, &module_url, filepath.clone(), true, false);
      assert!(result.is_ok());

      // module is already cached, should be ok even with `no_fetch`
      let result =
        get_source_code(&deno_dir, &module_url, filepath.clone(), true, true);
      assert!(result.is_ok());
    });
  }

  #[test]
  fn test_fetch_source_async_1() {
    use crate::tokio_util;
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let (_temp_dir, deno_dir) = test_setup();
      let module_url =
        Url::parse("http://127.0.0.1:4545/tests/subdir/mt_video_mp2t.t3.ts")
          .unwrap();
      let filepath = deno_dir
        .deps_http
        .join("127.0.0.1_PORT4545/tests/subdir/mt_video_mp2t.t3.ts");
      let headers_file_name = source_code_headers_filename(&filepath);

      let result = tokio_util::block_on(fetch_remote_source_async(
        &deno_dir,
        &module_url,
        &filepath,
      ));
      assert!(result.is_ok());
      let r = result.unwrap().unwrap();
      assert_eq!(r.source_code, b"export const loaded = true;\n");
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // matching ext, no .headers.json file created
      assert!(fs::read_to_string(&headers_file_name).is_err());

      // Modify .headers.json, make sure read from local
      save_source_code_headers(
        &filepath,
        Some("text/javascript".to_owned()),
        None,
      );
      let result2 = fetch_local_source(&deno_dir, &module_url, &filepath, None);
      assert!(result2.is_ok());
      let r2 = result2.unwrap().unwrap();
      assert_eq!(r2.source_code, b"export const loaded = true;\n");
      // Not MediaType::TypeScript due to .headers.json modification
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
    });
  }

  #[test]
  fn test_fetch_source_1() {
    use crate::tokio_util;
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let (_temp_dir, deno_dir) = test_setup();
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/mt_video_mp2t.t3.ts")
          .unwrap();
      let filepath = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/mt_video_mp2t.t3.ts");
      let headers_file_name = source_code_headers_filename(&filepath);

      let result = fetch_remote_source(&deno_dir, &module_url, &filepath);
      assert!(result.is_ok());
      let r = result.unwrap().unwrap();
      assert_eq!(r.source_code, "export const loaded = true;\n".as_bytes());
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // matching ext, no .headers.json file created
      assert!(fs::read_to_string(&headers_file_name).is_err());

      // Modify .headers.json, make sure read from local
      save_source_code_headers(
        &filepath,
        Some("text/javascript".to_owned()),
        None,
      );
      let result2 = fetch_local_source(&deno_dir, &module_url, &filepath, None);
      assert!(result2.is_ok());
      let r2 = result2.unwrap().unwrap();
      assert_eq!(r2.source_code, "export const loaded = true;\n".as_bytes());
      // Not MediaType::TypeScript due to .headers.json modification
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
    });
  }

  #[test]
  fn test_fetch_source_2() {
    use crate::tokio_util;
    // http_util::fetch_sync_string requires tokio
    tokio_util::init(|| {
      let (_temp_dir, deno_dir) = test_setup();
      let module_url =
        Url::parse("http://localhost:4545/tests/subdir/no_ext").unwrap();
      let filepath = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/no_ext");
      let result = fetch_remote_source(&deno_dir, &module_url, &filepath);
      assert!(result.is_ok());
      let r = result.unwrap().unwrap();
      assert_eq!(r.source_code, "export const loaded = true;\n".as_bytes());
      assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
      // no ext, should create .headers.json file
      assert_eq!(
        get_source_code_headers(&filepath).mime_type.unwrap(),
        "text/typescript"
      );

      let module_url_2 =
        Url::parse("http://localhost:4545/tests/subdir/mismatch_ext.ts")
          .unwrap();
      let filepath_2 = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/mismatch_ext.ts");
      let result_2 = fetch_remote_source(&deno_dir, &module_url_2, &filepath_2);
      assert!(result_2.is_ok());
      let r2 = result_2.unwrap().unwrap();
      assert_eq!(r2.source_code, "export const loaded = true;\n".as_bytes());
      assert_eq!(&(r2.media_type), &msg::MediaType::JavaScript);
      // mismatch ext, should create .headers.json file
      assert_eq!(
        get_source_code_headers(&filepath_2).mime_type.unwrap(),
        "text/javascript"
      );

      // test unknown extension
      let module_url_3 =
        Url::parse("http://localhost:4545/tests/subdir/unknown_ext.deno")
          .unwrap();
      let filepath_3 = deno_dir
        .deps_http
        .join("localhost_PORT4545/tests/subdir/unknown_ext.deno");
      let result_3 = fetch_remote_source(&deno_dir, &module_url_3, &filepath_3);
      assert!(result_3.is_ok());
      let r3 = result_3.unwrap().unwrap();
      assert_eq!(r3.source_code, "export const loaded = true;\n".as_bytes());
      assert_eq!(&(r3.media_type), &msg::MediaType::TypeScript);
      // unknown ext, should create .headers.json file
      assert_eq!(
        get_source_code_headers(&filepath_3).mime_type.unwrap(),
        "text/typescript"
      );
    });
  }

  #[test]
  fn test_fetch_source_3() {
    // only local, no http_util::fetch_sync_string called
    let (_temp_dir, deno_dir) = test_setup();
    let cwd = std::env::current_dir().unwrap();
    let module_url =
      Url::parse("http://example.com/mt_text_typescript.t1.ts").unwrap();
    let filepath = cwd.join("tests/subdir/mt_text_typescript.t1.ts");

    let result = fetch_local_source(&deno_dir, &module_url, &filepath, None);
    assert!(result.is_ok());
    let r = result.unwrap().unwrap();
    assert_eq!(r.source_code, "export const loaded = true;\n".as_bytes());
    assert_eq!(&(r.media_type), &msg::MediaType::TypeScript);
  }

  #[test]
  fn test_fetch_source_file() {
    let (_temp_dir, deno_dir) = test_setup();

    tokio_util::init(|| {
      // Test failure case.
      let specifier =
        ModuleSpecifier::resolve_url(file_url!("/baddir/hello.ts")).unwrap();
      let r = deno_dir.fetch_source_file(&specifier, true, false);
      assert!(r.is_err());

      // Assuming cwd is the deno repo root.
      let specifier =
        ModuleSpecifier::resolve_url_or_path("js/main.ts").unwrap();
      let r = deno_dir.fetch_source_file(&specifier, true, false);
      assert!(r.is_ok());
    })
  }

  #[test]
  fn test_fetch_source_file_1() {
    /*recompile ts file*/
    let (_temp_dir, deno_dir) = test_setup();

    tokio_util::init(|| {
      // Test failure case.
      let specifier =
        ModuleSpecifier::resolve_url(file_url!("/baddir/hello.ts")).unwrap();
      let r = deno_dir.fetch_source_file(&specifier, false, false);
      assert!(r.is_err());

      // Assuming cwd is the deno repo root.
      let specifier =
        ModuleSpecifier::resolve_url_or_path("js/main.ts").unwrap();
      let r = deno_dir.fetch_source_file(&specifier, false, false);
      assert!(r.is_ok());
    })
  }

  // https://github.com/denoland/deno/blob/golang/os_test.go#L16-L87
  #[test]
  fn test_url_to_deps_path_1() {
    let (_temp_dir, deno_dir) = test_setup();

    let test_cases = [
      (
        file_url!("/Users/rld/go/src/github.com/denoland/deno/testdata/subdir/print_hello.ts"),
        add_root!("/Users/rld/go/src/github.com/denoland/deno/testdata/subdir/print_hello.ts"),
      ),
      (
        file_url!("/Users/rld/go/src/github.com/denoland/deno/testdata/001_hello.js"),
        add_root!("/Users/rld/go/src/github.com/denoland/deno/testdata/001_hello.js"),
      ),
      (
        file_url!("/Users/rld/src/deno/hello.js"),
        add_root!("/Users/rld/src/deno/hello.js"),
      ),
      (
        file_url!("/this/module/got/imported.js"),
        add_root!("/this/module/got/imported.js"),
      ),
    ];
    for &test in test_cases.iter() {
      let url = Url::parse(test.0).unwrap();
      let filename = deno_dir.url_to_deps_path(&url).unwrap();
      assert_eq!(filename.to_str().unwrap().to_string(), test.1);
    }
  }

  #[test]
  fn test_url_to_deps_path_2() {
    let (_temp_dir, deno_dir) = test_setup();

    let specifier =
      Url::parse("http://localhost:4545/testdata/subdir/print_hello.ts")
        .unwrap();
    let expected_filename = normalize_to_str(
      deno_dir
        .deps_http
        .join("localhost_PORT4545/testdata/subdir/print_hello.ts")
        .as_ref(),
    );

    let filename = deno_dir.url_to_deps_path(&specifier).unwrap();
    assert_eq!(filename.to_str().unwrap().to_string(), expected_filename);
  }

  #[test]
  fn test_resolve_module_3() {
    let (_temp_dir, deno_dir) = test_setup();

    // unsupported schemes
    let test_cases = [
      "ftp://localhost:4545/testdata/subdir/print_hello.ts",
      "blob:https://whatwg.org/d0360e2f-caee-469f-9a2f-87d5b0456f6f",
    ];

    for &test in test_cases.iter() {
      let url = Url::parse(test).unwrap();
      assert_eq!(
        deno_dir.url_to_deps_path(&url).unwrap_err().kind(),
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
