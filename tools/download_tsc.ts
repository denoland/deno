#!/usr/bin/env -S deno run --allow-read --allow-write --allow-env --allow-run
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

// Pre-downloads the pinned native TypeScript compiler that `deno check` uses,
// into a cache directory, so CI (and local test runs) don't re-download the
// ~20MB compiler for every test's fresh `DENO_DIR`.
//
// It warms the cache by type-checking a trivial file with the built `deno`
// binary (which downloads the compiler via its normal path), then locates the
// resulting `tsc` binary. On success it prints the binary's path and, when run
// under GitHub Actions, appends `DENO_TSC_BIN=<path>` to `$GITHUB_ENV` so the
// subsequent test step points every `deno check` at it.
//
//   deno run -A tools/download_tsc.ts [cache_deno_dir]
//
// For local `cargo test` runs of the `deno check` tests, export the path first:
//
//   export DENO_TSC_BIN=$(deno run -A tools/download_tsc.ts)
//
// The `deno` binary to use is taken from the `DENO_BIN` env var, else the first
// of `./target/release/deno` or `./target/debug/deno` that exists.

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

function findTscBin(cacheDenoDir: string): string | undefined {
  // Layout: `<deno_dir>/tsc/<version>/<platform>/lib/tsc`.
  const tscRoot = `${cacheDenoDir}/tsc`;
  let versions: Deno.DirEntry[];
  try {
    versions = [...Deno.readDirSync(tscRoot)];
  } catch {
    return undefined;
  }
  for (const version of versions) {
    if (!version.isDirectory) continue;
    for (const platform of Deno.readDirSync(`${tscRoot}/${version.name}`)) {
      if (!platform.isDirectory) continue;
      const bin = `${tscRoot}/${version.name}/${platform.name}/lib/tsc${exe}`;
      try {
        Deno.statSync(bin);
        return bin;
      } catch {
        // keep looking
      }
    }
  }
  return undefined;
}

const cacheDenoDir = absolute(Deno.args[0] ?? "./target/.native_tsc/deno_dir");

let tscBin = findTscBin(cacheDenoDir);
if (!tscBin) {
  const denoBin = resolveDenoBin();
  const warmDir = Deno.makeTempDirSync();
  Deno.writeTextFileSync(`${warmDir}/mod.ts`, "export {};\n");
  // Type-checking a trivial file downloads the compiler into `cacheDenoDir`.
  // The exit code is irrelevant; the compiler is fetched regardless.
  new Deno.Command(denoBin, {
    args: ["check", "mod.ts"],
    cwd: warmDir,
    env: { DENO_DIR: cacheDenoDir },
    stdout: "null",
    stderr: "null",
  }).outputSync();
  tscBin = findTscBin(cacheDenoDir);
}

if (!tscBin) {
  console.error("failed to download the native TypeScript compiler");
  Deno.exit(1);
}

tscBin = Deno.realPathSync(tscBin);
console.log(tscBin);

const githubEnv = Deno.env.get("GITHUB_ENV");
if (githubEnv) {
  Deno.writeTextFileSync(githubEnv, `DENO_TSC_BIN=${tscBin}\n`, {
    append: true,
  });
}
