// Copyright 2018-2026 the Deno authors. MIT license.
//! Head-to-head microbenchmark of the legacy clap flag parser vs the
//! hand-written `deno_cli_parser`, built into the same binary (same profile /
//! LTO) so the comparison is free of the runtime-startup and build confounders
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

  // Legacy clap parser (note: `bare_run` still hits clap's fast-path, which
  // skips building the command tree; the other two build clap_root()).
  let mut g = c.benchmark_group("clap");
  for (name, input) in inputs {
    g.bench_function(name, |b| {
      b.iter_batched(
        || input.clone(),
        |args| {
          black_box(deno::clap_flags_from_vec_with_initial_cwd(
            black_box(args),
            None,
          ))
        },
        BatchSize::SmallInput,
      );
    });
  }
  g.finish();

  // Hand-written parser (production path).
  let mut g = c.benchmark_group("new_parser");
  for (name, input) in inputs {
    g.bench_function(name, |b| {
      b.iter_batched(
        || input.clone(),
        |args| black_box(deno::flags_from_vec_new(black_box(args), None)),
        BatchSize::SmallInput,
      );
    });
  }
  g.finish();
}

criterion_group!(flags, benches);
criterion_main!(flags);
