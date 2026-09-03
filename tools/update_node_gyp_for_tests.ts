#!/usr/bin/env -S deno run -ERNW --allow-sys

// Copyright 2018-2026 the Deno authors. MIT license.

// This script updates `test/testdata/assets/node-gyp/*` files that
// are used by the test registry.

import { create, extract } from "npm:tar@6.2.1";
import { Readable } from "node:stream";
import { join } from "jsr:@std/path@1.1.4";
import { createGzip } from "node:zlib";
import { createWriteStream } from "node:fs";

let version = Deno.args[0];

if (!version) {
  throw new Error("expected node version as arg, e.g. v20.11.1");
}

version = version.startsWith("v") ? version : "v" + version;

const assetsDir = "./tests/testdata/assets/node-gyp";

async function download(url: string): Promise<ReadableStream<Uint8Array>> {
  const response = await fetch(url);
  if (!response.ok || !response.body) {
    throw new Error(`failed to download ${url}: ${response.status}`);
  }
  return response.body;
}

function writeTarGz(
  cwd: string,
  entries: string[],
  outFile: string,
): Promise<void> {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  create({ cwd }, entries)
    .pipe(createGzip())
    .pipe(createWriteStream(outFile))
    .on("close", () => resolve())
    .on("error", reject);
  return promise;
}

// The headers tarball, minus the bundled OpenSSL headers which are large and
// unused by the tests.
{
  const temp = await Deno.makeTempDir();
  const body = await download(
    `https://nodejs.org/dist/${version}/node-${version}-headers.tar.gz`,
  );

  const extracted = Promise.withResolvers<void>();
  // deno-lint-ignore no-explicit-any
  Readable.fromWeb(body as any)
    .pipe(extract({ cwd: temp }))
    .once("close", () => extracted.resolve());
  await extracted.promise;

  await Deno.remove(
    join(temp, `node-${version}`, "include", "node", "openssl"),
    { recursive: true },
  );

  await writeTarGz(
    temp,
    [`node-${version}`],
    `${assetsDir}/node-${version}-headers.tar.gz`,
  );
}

// `node.lib` for the Windows targets, which node-gyp links against.
for (const platform of ["win-x64", "win-arm64"]) {
  const temp = await Deno.makeTempDir();
  const body = await download(
    `https://nodejs.org/dist/${version}/${platform}/node.lib`,
  );
  using file = await Deno.create(join(temp, "node.lib"));
  await body.pipeTo(file.writable, { preventClose: true });

  await writeTarGz(
    temp,
    ["node.lib"],
    `${assetsDir}/${version}__${platform}__node.lib.tar.gz`,
  );
}
