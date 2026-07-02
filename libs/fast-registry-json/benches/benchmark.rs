// Copyright 2018-2026 the Deno authors. MIT license.

use std::hint::black_box;
use std::path::PathBuf;

use deno_npm::registry::{NpmPackageInfo, NpmPackageVersionInfo};
use divan::Bencher;
use rustc_hash::FxHashMap;

fn main() {
  // Run registered benchmarks.
  divan::main();
}

macro_rules! bench_parse_pluck {
    ($name: ident, $package: expr, $version: expr) => {
        paste::paste! {
            #[divan::bench]
            fn [<bench_pluck_versions_ $name>](b: Bencher) {
                let input = read_packument($package);
                b.bench(|| {
                    black_box({
                        let versions = fast_registry_json::pluck_versions(&input).unwrap();
                        let mut map = FxHashMap::with_capacity_and_hasher(
                            versions.versions.len(),
                            Default::default(),
                        );
                        for (version, range) in
                            versions.versions.into_iter().zip(versions.version_ranges)
                        {
                            map.insert(version, range);
                        }
                        let &(start, end) = map.get($version).unwrap();
                        let info: NpmPackageVersionInfo =
                            serde_json::from_str(&input[start as usize..end as usize]).unwrap();
                        assert_eq!(info.version.to_string(), $version);
                        info
                    })
                });
            }

            #[divan::bench]
            fn [<bench_parse_versions_ $name>](b: Bencher) {
                let input = read_packument($package);
                let version = deno_semver::Version::parse_from_npm($version).unwrap();
                b.bench(|| {
                    black_box({
                        let info: NpmPackageInfo = serde_json::from_str(&input).unwrap();
                        let version_info = info.versions.get(&version).cloned().unwrap();
                        assert_eq!(version_info.version.to_string(), $version);
                        version_info
                    })
                });
            }

            #[divan::bench]
            fn [<bench_packument_index_ $name>](b: Bencher) {
                let input = read_packument($package);
                b.bench(|| {
                    black_box(fast_registry_json::pluck_packument_index(&input).unwrap())
                });
            }

            #[divan::bench]
            fn [<bench_packument_index_deser_ $name>](b: Bencher) {
                let input = read_packument($package);
                b.bench(|| {
                    black_box({
                        let index = fast_registry_json::pluck_packument_index(&input).unwrap();
                        let range = index
                            .versions
                            .iter()
                            .zip(index.version_ranges)
                            .find_map(|(version, range)| (*version == $version).then_some(range))
                            .unwrap();
                        let info: NpmPackageVersionInfo =
                            serde_json::from_str(&input[range.0 as usize..range.1 as usize])
                                .unwrap();
                        assert_eq!(info.version.to_string(), $version);
                        info
                    })
                });
            }
        }
    };
}

#[allow(
  clippy::disallowed_methods,
  reason = "benchmark fixture discovery uses host env vars and filesystem"
)]
fn read_packument(package_name: &str) -> String {
  let path = registry_path(package_name);
  std::fs::read_to_string(&path).unwrap_or_else(|err| {
    panic!(
      "failed reading benchmark packument {}: {err}\n\
       set FAST_REGISTRY_JSON_PACKUMENT_DIR to a directory containing \
       <package>/registry.json files, or set DENO_DIR to a populated Deno cache",
      path.display()
    )
  })
}

#[allow(
  clippy::disallowed_methods,
  reason = "benchmark fixture discovery uses host env vars and filesystem"
)]
fn registry_path(package_name: &str) -> PathBuf {
  if let Ok(dir) = std::env::var("FAST_REGISTRY_JSON_PACKUMENT_DIR") {
    return PathBuf::from(dir).join(package_name).join("registry.json");
  }
  if let Ok(deno_dir) = std::env::var("DENO_DIR") {
    return PathBuf::from(deno_dir)
      .join("npm")
      .join("registry.npmjs.org")
      .join(package_name)
      .join("registry.json");
  }
  panic!(
    "set FAST_REGISTRY_JSON_PACKUMENT_DIR or DENO_DIR before running \
     fast-registry-json benchmarks"
  );
}

bench_parse_pluck!(next, "next", "15.0.0-canary.202");
bench_parse_pluck!(prisma, "@prisma/client", "5.1.0-dev.64");
bench_parse_pluck!(node_pty, "node-pty", "1.0.0");
bench_parse_pluck!(client_only, "client-only", "0.0.1");
bench_parse_pluck!(drizzle_orm, "drizzle-orm", "0.29.0");
bench_parse_pluck!(drizzle_kit, "drizzle-kit", "0.29.0");
