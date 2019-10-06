// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::deno_error::DenoError;
use crate::version;
use deno::ErrBox;
use futures::{future, Future};
use reqwest;
use reqwest::header::HeaderMap;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::LOCATION;
use reqwest::header::USER_AGENT;
use reqwest::r#async::Client;
use reqwest::RedirectPolicy;
use url::Url;

/// Create new instance of async reqwest::Client. This client supports
/// proxies and doesn't follow redirects.
pub fn get_client() -> Client {
  let mut headers = HeaderMap::new();
  headers.insert(
    USER_AGENT,
    format!("Deno/{}", version::DENO).parse().unwrap(),
  );
  Client::builder()
    .redirect(RedirectPolicy::none())
    .default_headers(headers)
    .use_sys_proxy()
    .build()
    .unwrap()
}

/// Construct the next uri based on base uri and location header fragment
/// See <https://tools.ietf.org/html/rfc3986#section-4.2>
fn resolve_url_from_location(base_url: &Url, location: &str) -> Url {
  if location.starts_with("http://") || location.starts_with("https://") {
    // absolute uri
    Url::parse(location).expect("provided redirect url should be a valid url")
  } else if location.starts_with("//") {
    // "//" authority path-abempty
    Url::parse(&format!("{}:{}", base_url.scheme(), location))
      .expect("provided redirect url should be a valid url")
  } else if location.starts_with('/') {
    // path-absolute
    base_url
      .join(location)
      .expect("provided redirect url should be a valid url")
  } else {
    // assuming path-noscheme | path-empty
    let base_url_path_str = base_url.path().to_owned();
    // Pop last part or url (after last slash)
    let segs: Vec<&str> = base_url_path_str.rsplitn(2, '/').collect();
    let new_path = format!("{}/{}", segs.last().unwrap_or(&""), location);
    base_url
      .join(&new_path)
      .expect("provided redirect url should be a valid url")
  }
}

#[derive(Debug, PartialEq)]
pub enum FetchOnceResult {
  // (code, maybe_content_type)
  Code(String, Option<String>),
  Redirect(Url),
}

/// Asynchronously fetchs the given HTTP URL one pass only.
/// If no redirect is present and no error occurs,
/// yields Code(code, maybe_content_type).
/// If redirect occurs, does not follow and
/// yields Redirect(url).
pub fn fetch_string_once(
  url: &Url,
) -> impl Future<Item = FetchOnceResult, Error = ErrBox> {
  type FetchAttempt = (Option<String>, Option<String>, Option<FetchOnceResult>);

  let url = url.clone();
  let client = get_client();

  client
    .get(url.clone())
    .send()
    .map_err(ErrBox::from)
    .and_then(
      move |mut response| -> Box<
        dyn Future<Item = FetchAttempt, Error = ErrBox> + Send,
      > {
        if response.status().is_redirection() {
          let location_string = response.headers()
            .get(LOCATION)
            .expect("url redirection should provide 'location' header")
            .to_str()
            .unwrap();

          debug!("Redirecting to {:?}...", &location_string);
          let new_url = resolve_url_from_location(&url, location_string);
          // Boxed trait object turns out to be the savior for 2+ types yielding same results.
          return Box::new(future::ok(None).join3(
            future::ok(None),
            future::ok(Some(FetchOnceResult::Redirect(new_url))),
          ));
        }

        if response.status().is_client_error() || response.status().is_server_error() {
          return Box::new(future::err(DenoError::new(
            deno_error::ErrorKind::Other,
            format!("Import '{}' failed: {}", &url, response.status()),
          ).into()));
        }

        let content_type = response
          .headers()
          .get(CONTENT_TYPE)
          .map(|content_type| content_type.to_str().unwrap().to_owned());

        let body = response
          .text()
          .map_err(ErrBox::from);

        Box::new(
          Some(body).join3(future::ok(content_type), future::ok(None))
        )
      }
    )
    .and_then(move |(maybe_code, maybe_content_type, maybe_redirect)| {
      if let Some(redirect) = maybe_redirect {
        future::ok(redirect)
      } else {
        // maybe_code should always contain code here!
        future::ok(FetchOnceResult::Code(
          maybe_code.unwrap(),
          maybe_content_type,
        ))
      }
    })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tokio_util;

  #[test]
  fn test_fetch_sync_string() {
    let http_server_guard = crate::test_util::http_server();
    // Relies on external http server. See tools/http_server.py
    let url = Url::parse("http://127.0.0.1:4545/package.json").unwrap();

    let fut = fetch_string_once(&url).then(|result| match result {
      Ok(FetchOnceResult::Code(code, maybe_content_type)) => {
        assert!(!code.is_empty());
        assert_eq!(maybe_content_type, Some("application/json".to_string()));
        Ok(())
      }
      _ => panic!(),
    });

    tokio_util::run(fut);
    drop(http_server_guard);
  }

  #[test]
  fn test_fetch_string_once_with_redirect() {
    let http_server_guard = crate::test_util::http_server();
    // Relies on external http server. See tools/http_server.py
    let url = Url::parse("http://127.0.0.1:4546/package.json").unwrap();
    // Dns resolver substitutes `127.0.0.1` with `localhost`
    let target_url = Url::parse("http://localhost:4545/package.json").unwrap();
    let fut = fetch_string_once(&url).then(move |result| match result {
      Ok(FetchOnceResult::Redirect(url)) => {
        assert_eq!(url, target_url);
        Ok(())
      }
      _ => panic!(),
    });

    tokio_util::run(fut);
    drop(http_server_guard);
  }

  #[test]
  fn test_resolve_url_from_location_full_1() {
    let url = "http://deno.land".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "http://golang.org");
    assert_eq!(new_uri.host_str().unwrap(), "golang.org");
  }

  #[test]
  fn test_resolve_url_from_location_full_2() {
    let url = "https://deno.land".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "https://golang.org");
    assert_eq!(new_uri.host_str().unwrap(), "golang.org");
  }

  #[test]
  fn test_resolve_url_from_location_relative_1() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "//rust-lang.org/en-US");
    assert_eq!(new_uri.host_str().unwrap(), "rust-lang.org");
    assert_eq!(new_uri.path(), "/en-US");
  }

  #[test]
  fn test_resolve_url_from_location_relative_2() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "/y");
    assert_eq!(new_uri.host_str().unwrap(), "deno.land");
    assert_eq!(new_uri.path(), "/y");
  }

  #[test]
  fn test_resolve_url_from_location_relative_3() {
    let url = "http://deno.land/x".parse::<Url>().unwrap();
    let new_uri = resolve_url_from_location(&url, "z");
    assert_eq!(new_uri.host_str().unwrap(), "deno.land");
    assert_eq!(new_uri.path(), "/z");
  }
}
