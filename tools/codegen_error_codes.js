#!/usr/bin/env -S deno run --allow-net=raw.githubusercontent.com
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

async function windowsMappings() {
  const resp = await fetch(
    "https://raw.githubusercontent.com/libuv/libuv/26b2e5dbb6301756644d6e4cf6ca9c49c00513d3/src/win/error.c",
  );
  const text = await resp.text();
  return [...text.matchAll(
    /case (.*?):\s+return UV_(.*?);/g,
  )].map((m) => [m[1], m[2]]);
}

async function windowsCodes() {
  const resp = await fetch(
    "https://raw.githubusercontent.com/rust-lang/rust/c3190c1eb4c982b1d419ae0632bad07a3b306b48/library/std/src/sys/windows/c/errors.rs",
  );
  const text = await resp.text();
  const WSABASEERR = 10000;
  return Object.fromEntries([
    ...[...text.matchAll(
      /pub const (.*?): DWORD = (\d+);/g,
    )].map((m) => [m[1], Number(m[2])]),
    ...[...text.matchAll(
      /pub const (.*?): c_int = WSABASEERR \+ (\d+);/g,
    )].map((m) => [m[1], Number(m[2]) + WSABASEERR]),
  ]);
}

async function unixMappings() {
  const resp = await fetch(
    "https://raw.githubusercontent.com/libuv/libuv/3e90bc76b036124c2a94f9bf006af527755271cf/include/uv/errno.h",
  );
  const text = await resp.text();
  return [...text.matchAll(
    /if defined\((E.*?)\)/g,
  )].map((m) => [m[1], m[1]]);
}

function codegenUnix(unixPairs) {
  return `
#[cfg(unix)]
fn get_os_error_code(errno: i32) -> &'static str {
  match errno {
    ${unixPairs.map((p) => `libc::${p[0]} => "${p[0]}",`).join("\n    ")}
    _ => "",
  }
}`.trim();
}

function codegenWin(winPairs, winCodes) {
  return `
#[cfg(windows)]
fn get_os_error_code(errno: i32) -> &'static str {
  match errno {
    ${
    winPairs.map((p) => `${winCodes[p[0]]} => "${p[1]}", // ${p[0]}`).join(
      "\n    ",
    )
  }
    _ => "",
  }
}`.trim();
}

function intersection(a, b) {
  const setA = new Set(a);
  const setB = new Set(b);
  return new Set([...setA].filter((x) => setB.has(x)));
}

async function codegen() {
  // Raw pairs
  const rawWin = await windowsMappings();
  const rawUnix = await unixMappings();
  // Windows code name to number mapping
  const winCodes = await windowsCodes();

  // Only keep common error codes
  const common = intersection(
    rawWin.map((p) => p[1]),
    rawUnix.map((p) => p[1]),
  );
  const blocked = new Set(["ECHARSET"]);
  const win = rawWin.filter((p) => common.has(p[1]) && !blocked.has(p[1]));
  const unix = rawUnix.filter((p) => common.has(p[1]) && !blocked.has(p[1]));

  console.log(codegenUnix(unix));
  console.log();
  console.log(codegenWin(win, winCodes));
}

await codegen();
