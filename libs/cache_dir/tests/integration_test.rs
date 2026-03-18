// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::disallowed_methods, reason = "test code")]

use std::collections::HashMap;
use std::path::Path;

use deno_cache_dir::CacheReadFileError;
use deno_cache_dir::Checksum;
use deno_cache_dir::GlobalHttpCache;
use deno_cache_dir::GlobalToLocalCopy;
use deno_cache_dir::HttpCache;
use deno_cache_dir::LocalHttpCache;
use deno_cache_dir::LocalLspHttpCache;
use deno_maybe_sync::new_rc;
use serde_json::json;
use sys_traits::impls::RealSys;
use tempfile::TempDir;
use url::Url;

fn jsr_url() -> Url {
  Url::parse("https://jsr.io/").unwrap()
}

#[test]
fn test_global_create_cache() {
  let dir = TempDir::new().unwrap();
  let cache_path = dir.path().join("foobar");
  // HttpCache should be created lazily on first use:
  // when zipping up a local project with no external dependencies
  // "$DENO_DIR/remote" is empty. When unzipping such project
  // "$DENO_DIR/remote" might not get restored and in situation
  // when directory is owned by root we might not be able
  // to create that directory. However if it's not needed it
  // doesn't make sense to return error in such specific scenarios.
  // For more details check issue:
  // https://github.com/denoland/deno/issues/5688
  let sys = RealSys;
  let cache = GlobalHttpCache::new(sys, cache_path.clone());
  assert!(!cache.dir_path().exists());
  let url = Url::parse("http://example.com/foo/bar.js").unwrap();
  cache.set(&url, Default::default(), b"hello world").unwrap();
  assert!(cache_path.is_dir());
  assert!(cache.local_path_for_url(&url).unwrap().is_file());
}

#[test]
fn test_global_get_set() {
  let dir = TempDir::new().unwrap();
  let sys = RealSys;
  let cache = GlobalHttpCache::new(sys, dir.path().to_path_buf());
  let url = Url::parse("https://deno.land/x/welcome.ts").unwrap();
  let mut headers = HashMap::new();
  headers.insert(
    "content-type".to_string(),
    "application/javascript".to_string(),
  );
  headers.insert("etag".to_string(), "as5625rqdsfb".to_string());
  let content = b"Hello world";
  cache.set(&url, headers, content).unwrap();
  let key = cache.cache_item_key(&url).unwrap();
  let content = String::from_utf8(
    cache.get(&key, None).unwrap().unwrap().content.into_owned(),
  )
  .unwrap();
  let headers = cache.read_headers(&key).unwrap().unwrap();
  assert_eq!(content, "Hello world");
  assert_eq!(
    headers.get("content-type").unwrap(),
    "application/javascript"
  );
  assert_eq!(headers.get("etag").unwrap(), "as5625rqdsfb");
  assert_eq!(headers.get("foobar"), None);
  let download_time = cache.read_download_time(&key).unwrap().unwrap();
  let elapsed = download_time.elapsed().unwrap();
  assert!(elapsed.as_secs() < 2, "Elapsed: {:?}", elapsed);
  let matching_checksum =
    "64ec88ca00b268e5ba1a35678a1b5316d212f4f366b2477232534a8aeca37f3c";
  // reading with checksum that matches
  {
    let found_content = cache
      .get(&key, Some(Checksum::new(matching_checksum)))
      .unwrap()
      .unwrap()
      .content;
    assert_eq!(found_content, content.as_bytes());
  }
  // reading with a checksum that doesn't match
  {
    let not_matching_checksum = "1234";
    let err = cache
      .get(&key, Some(Checksum::new(not_matching_checksum)))
      .err()
      .unwrap();
    let err = match err {
      CacheReadFileError::ChecksumIntegrity(err) => err,
      _ => unreachable!(),
    };
    assert_eq!(err.actual, matching_checksum);
    assert_eq!(err.expected, not_matching_checksum);
    assert_eq!(err.url, url);
  }
}

#[test]
fn test_local_global_cache() {
  let temp_dir = TempDir::new().unwrap();
  let global_cache_path = temp_dir.path().join("global");
  let local_cache_path = temp_dir.path().join("local");
  let sys = RealSys;
  let global_cache =
    new_rc(GlobalHttpCache::new(sys, global_cache_path.clone()));
  let local_cache = LocalHttpCache::new(
    local_cache_path.clone(),
    global_cache.clone(),
    GlobalToLocalCopy::Allow,
    jsr_url(),
  );

  let manifest_file_path = local_cache_path.join("manifest.json");
  // mapped url
  {
    let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([(
          "content-type".to_string(),
          "application/typescript".to_string(),
        )]),
        content.as_bytes(),
      )
      .unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    assert_eq!(
      String::from_utf8(
        local_cache
          .get(&key, None)
          .unwrap()
          .unwrap()
          .content
          .into_owned()
      )
      .unwrap(),
      content
    );
    let headers = local_cache.read_headers(&key).unwrap().unwrap();
    // won't have any headers because the content-type is derivable from the url
    assert_eq!(headers, HashMap::new());
    // no manifest file yet
    assert!(!manifest_file_path.exists());

    // now try deleting the global cache and we should still be able to load it
    std::fs::remove_dir_all(&global_cache_path).unwrap();
    assert_eq!(
      String::from_utf8(
        local_cache
          .get(&key, None)
          .unwrap()
          .unwrap()
          .content
          .into_owned()
      )
      .unwrap(),
      content
    );
  }

  // file that's directly mappable to a url
  {
    let content = "export const a = 1;";
    std::fs::write(local_cache_path.join("deno.land").join("main.js"), content)
      .unwrap();

    // now we should be able to read this file because it's directly mappable to a url
    let url = Url::parse("https://deno.land/main.js").unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    assert_eq!(
      String::from_utf8(
        local_cache
          .get(&key, None)
          .unwrap()
          .unwrap()
          .content
          .into_owned()
      )
      .unwrap(),
      content
    );
    let headers = local_cache.read_headers(&key).unwrap().unwrap();
    assert_eq!(headers, HashMap::new());
  }

  // now try a file with a different content-type header
  {
    let url =
      Url::parse("https://deno.land/x/different_content_type.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([(
          "content-type".to_string(),
          "application/javascript".to_string(),
        )]),
        content.as_bytes(),
      )
      .unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    assert_eq!(
      String::from_utf8(
        local_cache
          .get(&key, None)
          .unwrap()
          .unwrap()
          .content
          .into_owned()
      )
      .unwrap(),
      content
    );
    let headers = local_cache.read_headers(&key).unwrap().unwrap();
    assert_eq!(
      headers,
      HashMap::from([(
        "content-type".to_string(),
        "application/javascript".to_string(),
      )])
    );
    assert_eq!(
      read_manifest(&manifest_file_path),
      json!({
        "modules": {
          "https://deno.land/x/different_content_type.ts": {
            "headers": {
              "content-type": "application/javascript"
            }
          }
        }
      })
    );
    // delete the manifest file
    std::fs::remove_file(&manifest_file_path).unwrap();

    // Now try resolving the key again and the content type should still be application/javascript.
    // This is maintained because we hash the filename when the headers don't match the extension.
    let headers = local_cache.read_headers(&key).unwrap().unwrap();
    assert_eq!(
      headers,
      HashMap::from([(
        "content-type".to_string(),
        "application/javascript".to_string(),
      )])
    );
  }

  // reset the local cache
  std::fs::remove_dir_all(&local_cache_path).unwrap();
  let local_cache = LocalHttpCache::new(
    local_cache_path.clone(),
    global_cache.clone(),
    GlobalToLocalCopy::Allow,
    jsr_url(),
  );

  // now try caching a file with many headers
  {
    let url = Url::parse("https://deno.land/x/my_file.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([
          (
            "content-type".to_string(),
            "application/typescript".to_string(),
          ),
          ("x-typescript-types".to_string(), "./types.d.ts".to_string()),
          ("x-deno-warning".to_string(), "Stop right now.".to_string()),
          (
            "x-other-header".to_string(),
            "Thank you very much.".to_string(),
          ),
        ]),
        content.as_bytes(),
      )
      .unwrap();
    let check_output = |local_cache: &LocalHttpCache<_>| {
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache
            .get(&key, None)
            .unwrap()
            .unwrap()
            .content
            .into_owned()
        )
        .unwrap(),
        content
      );
      let headers = local_cache.read_headers(&key).unwrap().unwrap();
      assert_eq!(
        headers,
        HashMap::from([
          ("x-typescript-types".to_string(), "./types.d.ts".to_string(),),
          ("x-deno-warning".to_string(), "Stop right now.".to_string(),)
        ])
      );
      assert_eq!(
        read_manifest(&manifest_file_path),
        json!({
          "modules": {
            "https://deno.land/x/my_file.ts": {
              "headers": {
                "x-deno-warning": "Stop right now.",
                "x-typescript-types": "./types.d.ts"
              }
            }
          }
        })
      );
    };
    check_output(&local_cache);
    // now ensure it's the same when re-creating the cache
    check_output(&LocalHttpCache::new(
      local_cache_path.to_path_buf(),
      global_cache.clone(),
      GlobalToLocalCopy::Allow,
      jsr_url(),
    ));
  }

  // reset the local cache
  std::fs::remove_dir_all(&local_cache_path).unwrap();
  let local_cache = LocalHttpCache::new(
    local_cache_path.clone(),
    global_cache.clone(),
    GlobalToLocalCopy::Allow,
    jsr_url(),
  );

  // try a file that can't be mapped to the file system
  {
    {
      let url = Url::parse("https://deno.land/INVALID/Module.ts?dev").unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache
            .get(&key, None)
            .unwrap()
            .unwrap()
            .content
            .into_owned()
        )
        .unwrap(),
        content
      );
      let headers = local_cache.read_headers(&key).unwrap().unwrap();
      // won't have any headers because the content-type is derivable from the url
      assert_eq!(headers, HashMap::new());
    }

    // now try a file in the same directory, but that maps to the local filesystem
    {
      let url = Url::parse("https://deno.land/INVALID/module2.ts").unwrap();
      let content = "export const test = 4;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
        .unwrap();
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache
            .get(&key, None)
            .unwrap()
            .unwrap()
            .content
            .into_owned()
        )
        .unwrap(),
        content
      );
      assert!(
        local_cache_path
          .join("deno.land/#invalid_1ee01/module2.ts")
          .exists()
      );

      // ensure we can still read this file with a new local cache
      let local_cache = LocalHttpCache::new(
        local_cache_path.to_path_buf(),
        global_cache.clone(),
        GlobalToLocalCopy::Allow,
        jsr_url(),
      );
      assert_eq!(
        String::from_utf8(
          local_cache
            .get(&key, None)
            .unwrap()
            .unwrap()
            .content
            .into_owned()
        )
        .unwrap(),
        content
      );
    }

    assert_eq!(
      read_manifest(&manifest_file_path),
      json!({
        "modules": {
          "https://deno.land/INVALID/Module.ts?dev": {
          }
        },
        "folders": {
          "https://deno.land/INVALID/": "deno.land/#invalid_1ee01",
        }
      })
    );
  }

  // reset the local cache
  std::fs::remove_dir_all(&local_cache_path).unwrap();
  let local_cache = LocalHttpCache::new(
    local_cache_path.clone(),
    global_cache.clone(),
    GlobalToLocalCopy::Allow,
    jsr_url(),
  );

  // now try a redirect
  {
    let url = Url::parse("https://deno.land/redirect.ts").unwrap();
    global_cache
      .set(
        &url,
        HashMap::from([("location".to_string(), "./x/mod.ts".to_string())]),
        "Redirecting to other url...".as_bytes(),
      )
      .unwrap();
    let key = local_cache.cache_item_key(&url).unwrap();
    let headers = local_cache.read_headers(&key).unwrap().unwrap();
    assert_eq!(
      headers,
      HashMap::from([("location".to_string(), "./x/mod.ts".to_string())])
    );
    assert_eq!(
      read_manifest(&manifest_file_path),
      json!({
        "modules": {
          "https://deno.land/redirect.ts": {
            "headers": {
              "location": "./x/mod.ts"
            }
          }
        }
      })
    );
  }

  // reset the local cache
  std::fs::remove_dir_all(&local_cache_path).unwrap();
  let local_cache = LocalHttpCache::new(
    local_cache_path.clone(),
    global_cache.clone(),
    GlobalToLocalCopy::Allow,
    jsr_url(),
  );
  let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
  let matching_checksum =
    "5eadcbe625a8489347fc3b229ab66bdbcbdfecedf229dfe5d0a8a399dae6c005";
  let content = "export const test = 5;";
  global_cache
    .set(
      &url,
      HashMap::from([(
        "content-type".to_string(),
        "application/typescript".to_string(),
      )]),
      content.as_bytes(),
    )
    .unwrap();
  let key = local_cache.cache_item_key(&url).unwrap();
  // reading with a checksum that doesn't match
  // (ensure it doesn't match twice so we know it wasn't copied to the local cache)
  for _ in 0..2 {
    let not_matching_checksum = "1234";
    let err = local_cache
      .get(&key, Some(Checksum::new(not_matching_checksum)))
      .err()
      .unwrap();
    let err = match err {
      CacheReadFileError::ChecksumIntegrity(err) => err,
      _ => unreachable!(),
    };
    assert_eq!(err.actual, matching_checksum);
    assert_eq!(err.expected, not_matching_checksum);
    assert_eq!(err.url, url);
  }
  // reading with checksum that matches
  {
    let found_content = local_cache
      .get(&key, Some(Checksum::new(matching_checksum)))
      .unwrap()
      .unwrap()
      .content;
    assert_eq!(found_content, content.as_bytes());
  }
  // at this point the file should exist in the local cache and so the checksum will be ignored
  {
    let found_content = local_cache
      .get(&key, Some(Checksum::new("not matching")))
      .unwrap()
      .unwrap()
      .content;
    assert_eq!(found_content, content.as_bytes());
  }
}

fn read_manifest(path: &Path) -> serde_json::Value {
  let manifest = std::fs::read_to_string(path).unwrap();
  serde_json::from_str(&manifest).unwrap()
}

#[test]
fn test_lsp_local_cache() {
  let temp_dir = TempDir::new().unwrap();
  let global_cache_path = temp_dir.path().join("global");
  let local_cache_path = temp_dir.path().join("local");
  let sys = RealSys;
  let global_cache =
    new_rc(GlobalHttpCache::new(sys, global_cache_path.to_path_buf()));
  let local_cache = LocalHttpCache::new(
    local_cache_path.to_path_buf(),
    global_cache.clone(),
    GlobalToLocalCopy::Allow,
    jsr_url(),
  );
  let create_readonly_cache = || {
    LocalLspHttpCache::new(local_cache_path.to_path_buf(), global_cache.clone())
  };

  // mapped url
  {
    let url = Url::parse("https://deno.land/x/mod.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([(
          "content-type".to_string(),
          "application/typescript".to_string(),
        )]),
        content.as_bytes(),
      )
      .unwrap();
    // will be None because it's readonly
    {
      let readonly_local_cache = create_readonly_cache();
      let key = readonly_local_cache.cache_item_key(&url).unwrap();
      assert_eq!(readonly_local_cache.get(&key, None).unwrap(), None);
    }
    // populate it with the non-readonly local cache
    {
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache
            .get(&key, None)
            .unwrap()
            .unwrap()
            .content
            .into_owned()
        )
        .unwrap(),
        content
      );
    }
    // now the readonly cache will have it
    {
      let readonly_local_cache = create_readonly_cache();
      let key = readonly_local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          readonly_local_cache
            .get(&key, None)
            .unwrap()
            .unwrap()
            .content
            .into_owned()
        )
        .unwrap(),
        content
      );
    }

    {
      // check getting the file url works
      let readonly_local_cache = create_readonly_cache();
      let file_url = readonly_local_cache.get_file_url(&url);
      let expected = Url::from_directory_path(&local_cache_path)
        .unwrap()
        .join("deno.land/x/mod.ts")
        .unwrap();
      assert_eq!(file_url, Some(expected));

      // get the reverse mapping
      let mapping = readonly_local_cache.get_remote_url(
        local_cache_path
          .join("deno.land")
          .join("x")
          .join("mod.ts")
          .as_path(),
      );
      assert_eq!(mapping.as_ref(), Some(&url));
    }
  }

  // now try a file with a different content-type header
  {
    let url =
      Url::parse("https://deno.land/x/different_content_type.ts").unwrap();
    let content = "export const test = 5;";
    global_cache
      .set(
        &url,
        HashMap::from([(
          "content-type".to_string(),
          "application/javascript".to_string(),
        )]),
        content.as_bytes(),
      )
      .unwrap();
    // populate it with the non-readonly local cache
    {
      let key = local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          local_cache
            .get(&key, None)
            .unwrap()
            .unwrap()
            .content
            .into_owned()
        )
        .unwrap(),
        content
      );
    }
    {
      let readonly_local_cache = create_readonly_cache();
      let key = readonly_local_cache.cache_item_key(&url).unwrap();
      assert_eq!(
        String::from_utf8(
          readonly_local_cache
            .get(&key, None)
            .unwrap()
            .unwrap()
            .content
            .into_owned()
        )
        .unwrap(),
        content
      );

      let file_url = readonly_local_cache.get_file_url(&url).unwrap();
      let path = file_url.to_file_path().unwrap();
      assert!(path.exists());
      let mapping = readonly_local_cache.get_remote_url(&path);
      assert_eq!(mapping.as_ref(), Some(&url));
    }
  }

  // try http specifiers that can't be mapped to the file system
  {
    let urls = [
      "http://deno.land/INVALID/Module.ts?dev",
      "http://deno.land/INVALID/SubDir/Module.ts?dev",
    ];
    for url in urls {
      let url = Url::parse(url).unwrap();
      let content = "export const test = 5;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
        .unwrap();
      // populate it with the non-readonly local cache
      {
        let key = local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            local_cache
              .get(&key, None)
              .unwrap()
              .unwrap()
              .content
              .into_owned()
          )
          .unwrap(),
          content
        );
      }
      {
        let readonly_local_cache = create_readonly_cache();
        let key = readonly_local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            readonly_local_cache
              .get(&key, None)
              .unwrap()
              .unwrap()
              .content
              .into_owned()
          )
          .unwrap(),
          content
        );

        let file_url = readonly_local_cache.get_file_url(&url).unwrap();
        let path = file_url.to_file_path().unwrap();
        assert!(path.exists());
        let mapping = readonly_local_cache.get_remote_url(&path);
        assert_eq!(mapping.as_ref(), Some(&url));
      }
    }

    // now try a files in the same and sub directories, that maps to the local filesystem
    let urls = [
      "http://deno.land/INVALID/module2.ts",
      "http://deno.land/INVALID/SubDir/module3.ts",
      "http://deno.land/INVALID/SubDir/sub_dir/module4.ts",
    ];
    for url in urls {
      let url = Url::parse(url).unwrap();
      let content = "export const test = 4;";
      global_cache
        .set(&url, HashMap::new(), content.as_bytes())
        .unwrap();
      // populate it with the non-readonly local cache
      {
        let key = local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            local_cache
              .get(&key, None)
              .unwrap()
              .unwrap()
              .content
              .into_owned()
          )
          .unwrap(),
          content
        );
      }
      {
        let readonly_local_cache = create_readonly_cache();
        let key = readonly_local_cache.cache_item_key(&url).unwrap();
        assert_eq!(
          String::from_utf8(
            readonly_local_cache
              .get(&key, None)
              .unwrap()
              .unwrap()
              .content
              .into_owned()
          )
          .unwrap(),
          content
        );
        let file_url = readonly_local_cache.get_file_url(&url).unwrap();
        let path = file_url.to_file_path().unwrap();
        assert!(path.exists());
        let mapping = readonly_local_cache.get_remote_url(&path);
        assert_eq!(mapping.as_ref(), Some(&url));
      }

      // ensure we can still get this file with a new local cache
      let local_cache = create_readonly_cache();
      let file_url = local_cache.get_file_url(&url).unwrap();
      let path = file_url.to_file_path().unwrap();
      assert!(path.exists());
      let mapping = local_cache.get_remote_url(&path);
      assert_eq!(mapping.as_ref(), Some(&url));
    }
  }
}
