#!/usr/bin/env -S deno run -ERNW --allow-sys

// Copyright 2018-2025 the Deno authors. MIT license.

// This script updates `test/testdata/assets/node-gyp/*` files that
// are used by the test registry.

import { create, extract } from "npm:tar";
import { Readable } from "node:stream";
import { join } from "jsr:@std/path";
import { createGzip } from "node:zlib";
import { createWriteStream } from "node:fs";

let version = Deno.args[0];

if (!version) {
  throw new Error("expected node version as arg, e.g. v20.11.1");
}

version = version.startsWith("v") ? version : "v" + version;

const response = await fetch(
  `https://nodejs.org/dist/${version}/node-${version}-headers.tar.gz`,
);

if (!response.body) {
  throw new Error("expected response body");
}

const temp = await Deno.makeTempDir();

const p = Promise.withResolvers<void>();
Readable.fromWeb(response.body).pipe(
  extract({
    cwd: temp,
  }),
).once("close", (_r) => {
  p.resolve();
});

await p.promise;

await Deno.remove(join(temp, `node-${version}`, "include", "node", "openssl"), {
  recursive: true,
});

const stream = create({
  // file: `./node-${version}-headers.tar.gz`,
  // sync: true,
  onWriteEntry(entry) {
    if (entry.path.startsWith(temp.slice(1))) {
      entry.path = entry.path.slice(temp.length);
    }
  },
}, [join(temp, `node-${version}`)]);

const gzip = createGzip();
const fsStream = createWriteStream(
  `./tests/testdata/assets/node-gyp/node-${version}-headers.tar.gz`,
);
const p2 = Promise.withResolvers<void>();
stream.pipe(gzip).pipe(fsStream).on("close", (_) => {
  p2.resolve();
});

await p2.promise;
