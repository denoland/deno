// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::fs as deno_fs;
use std;
use std::str;
use std::str::FromStr;
use url::Url;

/// Convert paths supplied into full path.
/// If a path is invalid, we print out a warning
/// and ignore this path in the output.
pub fn resolve_paths(paths: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  for pathstr in paths.iter() {
    let result = deno_fs::resolve_from_cwd(pathstr);
    if result.is_err() {
      eprintln!("Unrecognized path to whitelist: {}", pathstr);
      continue;
    }
    let mut full_path = result.unwrap().1;
    // Remove trailing slash.
    if full_path.len() > 1 && full_path.ends_with('/') {
      full_path.pop();
    }
    out.push(full_path);
  }
  out
}

pub fn resolve_urls(urls: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  for urlstr in urls.iter() {
    let result = Url::from_str(urlstr);
    if result.is_err() {
      panic!("Bad Url: {}", urlstr);
    }
    let mut url = result.unwrap();
    url.set_fragment(None);
    let mut full_url = String::from(url.as_str());
    if full_url.len() > 1 && full_url.ends_with('/') {
      full_url.pop();
    }
    out.push(full_url);
  }
  out
}

/// This function expands "bare port" paths (eg. ":8080")
/// into full paths with hosts. It expands to such paths
/// into 3 paths with following hosts: `0.0.0.0:port`, `127.0.0.1:port` and `localhost:port`.
pub fn resolve_hosts(paths: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = vec![];
  for host_and_port in paths.iter() {
    let parts = host_and_port.split(':').collect::<Vec<&str>>();

    match parts.len() {
      // host only
      1 => {
        out.push(host_and_port.to_owned());
      }
      // host and port (NOTE: host might be empty string)
      2 => {
        let host = parts[0];
        let port = parts[1];

        if !host.is_empty() {
          out.push(host_and_port.to_owned());
          continue;
        }

        // we got bare port, let's add default hosts
        for host in ["0.0.0.0", "127.0.0.1", "localhost"].iter() {
          out.push(format!("{}:{}", host, port));
        }
      }
      _ => panic!("Bad host:port pair: {}", host_and_port),
    }
  }

  out
}
