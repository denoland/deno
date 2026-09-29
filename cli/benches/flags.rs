// Copyright 2018-2026 the Deno authors. MIT license.
//! Microbenchmark of the CLI flag-parsing entry point (`deno_cli_parser` plus
//! the CLI-side bare-run fast path), built into the same binary (same profile /
//! LTO) so the numbers are free of the runtime-startup and build confounders
//! that a `deno run` wall-clock benchmark has.

use std::ffi::OsString;
use std::hint::black_box;

use criterion::BatchSize;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;

fn osvec(args: &[&str]) -> Vec<OsString> {
  args.iter().map(OsString::from).collect()
}

fn benches(c: &mut Criterion) {
  let bare = osvec(&["deno", "run", "script.ts"]);
  let with_flags = osvec(&[
    "deno",
    "run",
    "-A",
    "--no-check",
    "script.ts",
    "arg1",
    "arg2",
  ]);
  let complex = osvec(&[
    "deno",
    "run",
    "--allow-read=/a,/b",
    "--allow-net=example.com",
    "--no-check",
    "--unstable-sloppy-imports",
    "--v8-flags=--max-old-space-size=4096",
    "--import-map=import_map.json",
    "--node-modules-dir=auto",
    "--reload",
    "script.ts",
    "arg1",
    "arg2",
  ]);
  let inputs = [
    ("bare_run", &bare),
    ("run_with_flags", &with_flags),
    ("complex", &complex),
  ];

  let mut g = c.benchmark_group("flags_from_vec");
  for (name, input) in inputs {
    g.bench_function(name, |b| {
      b.iter_batched(
        || input.clone(),
        |args| {
          black_box(deno::flags_from_vec_with_initial_cwd(
            black_box(args),
            None,
          ))
        },
        BatchSize::SmallInput,
      );
    });
  }
  g.finish();
}

criterion_group!(flags, benches);
criterion_main!(flags);
