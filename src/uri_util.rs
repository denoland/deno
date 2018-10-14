// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use http::uri;
use std::vec;

fn resolve_path(base: &str, path: &str) -> String {
  let full_path: String;
  if path == "" {
    full_path = String::from(base);
  } else if path.chars().next().unwrap() != '/' {
    let i = base.rfind("/").unwrap() + 1;
    full_path = String::from(&base[..i]) + path;
  } else {
    full_path = String::from(path);
  }

  if full_path == "" {
    return full_path;
  }

  let mut new_components: vec::Vec<String> = vec::Vec::new();
  for component in full_path.split("/") {
    if component == ".." {
      if !new_components.is_empty() {
        new_components.pop();
      }
    } else if component != "." {
      new_components.push(component.to_string());
    }
  }

  let last_component = full_path.split("/").last();
  if last_component == Some(".") || last_component == Some("..") {
    new_components.push("".to_string());
  }

  format!("/{}", new_components.join("/").trim_left_matches("/")).to_string()
}

pub fn _uri_join(
  base_uri: uri::Uri,
  ref_str: &str,
) -> Result<uri::Uri, uri::InvalidUriParts> {
  let ref_uri = match ref_str.parse::<uri::Uri>() {
    Ok(ref_uri) => ref_uri,
    Err(_) => ".".parse::<uri::Uri>().unwrap(),
  };
  let mut joined_parts = uri::Parts::default();
  if ref_uri.scheme_part() != None {
    let ref_uri_parts = ref_uri.into_parts();
    joined_parts.scheme = ref_uri_parts.scheme;
    joined_parts.authority = ref_uri_parts.authority;
    joined_parts.path_and_query = ref_uri_parts.path_and_query;
  } else {
    let base_uri_parts = base_uri.into_parts();
    joined_parts.scheme = base_uri_parts.scheme;
    joined_parts.authority = base_uri_parts.authority;

    let base_path_and_query = base_uri_parts.path_and_query.unwrap();
    let base_path = base_path_and_query.path();
    let joined_path = resolve_path(base_path, ref_str);
    let joined_path_and_query =
      uri::PathAndQuery::from_shared(joined_path.into()).unwrap();
    joined_parts.path_and_query = Some(joined_path_and_query);
  }

  uri::Uri::from_parts(joined_parts)
}

#[cfg(test)]
fn test_uri_join(base: &str, path: &str, expected: &str) {
  let base_uri = base.parse::<uri::Uri>().unwrap();
  let expected_uri = expected.parse::<uri::Uri>().unwrap();
  assert_eq!(expected_uri, _uri_join(base_uri, path).unwrap());
}

#[test]
fn test_uri_join_absolute_url_refer() {
  test_uri_join("http://foo.com?a=b", "https://bar.com/", "https://bar.com/");
  test_uri_join(
    "http://foo.com/",
    "https://bar.com/?a=b",
    "https://bar.com/?a=b",
  );
  test_uri_join("http://foo.com/", "https://bar.com/?", "https://bar.com/?");
}

#[test]
fn test_uri_join_multiple_slashes() {
  test_uri_join("http://foo.com/bar", "//baz", "http://foo.com/baz");
  test_uri_join(
    "http://foo.com/bar",
    "///baz/quux",
    "http://foo.com/baz/quux",
  );
}

#[test]
fn test_uri_join_current_directory() {
  test_uri_join("https://foo.com", ".", "https://foo.com/");
  test_uri_join("https://foo.com/bar", ".", "https://foo.com/");
  test_uri_join("https://foo.com/bar/", ".", "https://foo.com/bar/");
}

#[test]
fn test_uri_join_going_down() {
  test_uri_join("http://foo.com", "bar", "http://foo.com/bar");
  test_uri_join("http://foo.com/", "bar", "http://foo.com/bar");
  test_uri_join("http://foo.com/bar/baz", "quux", "http://foo.com/bar/quux");
}

#[test]
fn test_uri_join_going_up() {
  test_uri_join("http://foo.com/bar/baz", "../quux", "http://foo.com/quux");
  test_uri_join(
    "http://foo.com/bar/baz",
    "../../../../../quux",
    "http://foo.com/quux",
  );
  test_uri_join("http://foo.com/bar", "..", "http://foo.com/");
  test_uri_join("http://foo.com/bar/baz", "./..", "http://foo.com/");
}

#[test]
fn test_uri_join_dotdot_in_the_middle() {
  // ".." in the middle
  test_uri_join(
    "http://foo.com/bar/baz",
    "quux/dotdot/../tail",
    "http://foo.com/bar/quux/tail",
  );
  test_uri_join(
    "http://foo.com/bar/baz",
    "quux/./dotdot/../tail",
    "http://foo.com/bar/quux/tail",
  );
  test_uri_join(
    "http://foo.com/bar/baz",
    "quux/./dotdot/.././tail",
    "http://foo.com/bar/quux/tail",
  );
  test_uri_join(
    "http://foo.com/bar/baz",
    "quux/./dotdot/./../tail",
    "http://foo.com/bar/quux/tail",
  );
  test_uri_join(
    "http://foo.com/bar/baz",
    "quux/./dotdot/dotdot/././../../tail",
    "http://foo.com/bar/quux/tail",
  );
  test_uri_join(
    "http://foo.com/bar/baz",
    "quux/./dotdot/dotdot/./.././../tail",
    "http://foo.com/bar/quux/tail",
  );
  test_uri_join(
    "http://foo.com/bar/baz",
    "quux/./dotdot/dotdot/dotdot/./../../.././././tail",
    "http://foo.com/bar/quux/tail",
  );
  test_uri_join(
    "http://foo.com/bar/baz",
    "quux/./dotdot/../dotdot/../dot/./tail/..",
    "http://foo.com/bar/quux/dot/",
  );
}

#[test]
fn test_uri_join_remove_any_dot_segments() {
  // Remove any dot-segments prior to forming the target URI.
  // http://tools.ietf.org/html/rfc3986#section-5.2.4
  test_uri_join(
    "http://foo.com/dot/./dotdot/../foo/bar",
    "../baz",
    "http://foo.com/dot/baz",
  );
}

#[test]
fn test_uri_join_triple_dot() {
  // Triple dot isn't special
  test_uri_join("http://foo.com/bar", "...", "http://foo.com/...");
}

#[test]
fn test_uri_join_path_with_escaping() {
  test_uri_join("http://foo.com/foo%2fbar/", "../baz", "http://foo.com/baz");
  test_uri_join(
    "http://foo.com/1/2%2f/3%2f4/5",
    "../../a/b/c",
    "http://foo.com/1/a/b/c",
  );
  test_uri_join(
    "http://foo.com/1/2/3",
    "./a%2f../../b/..%2fc",
    "http://foo.com/1/2/b/..%2fc",
  );
  test_uri_join(
    "http://foo.com/1/2%2f/3%2f4/5",
    "./a%2f../b/../c",
    "http://foo.com/1/2%2f/3%2f4/a%2f../c",
  );
  test_uri_join("http://foo.com/foo%20bar/", "../baz", "http://foo.com/baz");
  test_uri_join(
    "http://foo.com/foo",
    "../bar%2fbaz",
    "http://foo.com/bar%2fbaz",
  );
  test_uri_join(
    "http://foo.com/foo%2dbar/",
    "./baz-quux",
    "http://foo.com/foo%2dbar/baz-quux",
  );
}
