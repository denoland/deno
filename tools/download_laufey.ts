#!/usr/bin/env -S deno run --allow-read --allow-write --allow-env --allow-run --allow-net
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

// Pre-downloads a "laufey" desktop backend that `deno desktop --backend=<x>`
// launches, into a cache directory, so CI (and local test runs) don't
// re-download the (100+ MB) backend archive for every test's fresh
// `DENO_DIR`.
//
// It warms the cache by compiling (not running) a trivial desktop app with
// the built `deno` binary, which downloads + packages the backend via its
// normal path. The test harness points every spec test at the same cache
// directory via `DENO_LAUFEY_CACHE_DIR` (see
// `test_util::native_laufey_cache_dir`), so this script only needs to warm
// the cache - it does not export the path into the env.
//
//   deno run -A tools/download_laufey.ts [backend] [cache_dir]
//
// For local `cargo test` runs, running it once (with the default backend
// and cache directory) is enough; the harness points every test at
// `target/.native_laufey`. To point tests at a different cache instead,
// export it explicitly:
//
//   export DENO_LAUFEY_CACHE_DIR=/path/to/cache
//
// The `deno` binary to use is taken from the `DENO_BIN` env var, else the
// first of `./target/release/deno` or `./target/debug/deno` that exists.

const exe = Deno.build.os === "windows" ? ".exe" : "";

// Absolute paths are required because the download runs the compiler in a
// different working directory, against which relative paths would resolve.
function absolute(p: string): string {
  if (p.startsWith("/") || /^[A-Za-z]:[\\/]/.test(p)) return p;
  return `${Deno.cwd()}/${p}`;
}

function resolveDenoBin(): string {
  const fromEnv = Deno.env.get("DENO_BIN");
  if (fromEnv) return Deno.realPathSync(fromEnv);
  for (const profile of ["release", "debug"]) {
    const candidate = `./target/${profile}/deno${exe}`;
    try {
      return Deno.realPathSync(candidate);
    } catch {
      // try the next profile
    }
  }
  throw new Error(
    "could not find a built deno binary; set DENO_BIN or build deno first",
  );
}

const backend = Deno.args[0] ?? "cef";
const cacheDir = absolute(Deno.args[1] ?? "./target/.native_laufey");

const denoBin = resolveDenoBin();
const warmDir = Deno.makeTempDirSync();
Deno.writeTextFileSync(
  `${warmDir}/main.ts`,
  `Deno.serve(() => new Response("ok"));\n`,
);

console.error(`Downloading laufey "${backend}" backend into ${cacheDir}...`);
// `deno desktop` without `--hmr` only compiles + packages the app - it never
// launches the backend - so this is safe to run headless.
const output = new Deno.Command(denoBin, {
  args: [
    "desktop",
    `--backend=${backend}`,
    "--output",
    `${warmDir}/app`,
    `${warmDir}/main.ts`,
  ],
  cwd: warmDir,
  env: { DENO_LAUFEY_CACHE_DIR: cacheDir },
  stdout: "inherit",
  stderr: "inherit",
}).outputSync();

if (!output.success) {
  console.error(`failed to download the laufey "${backend}" backend`);
  Deno.exit(1);
}

console.log(cacheDir);
