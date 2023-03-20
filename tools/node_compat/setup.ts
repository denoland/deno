#!/usr/bin/env -S deno run --allow-read=. --allow-write=. --allow-net=nodejs.org
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/** This script downloads Node.js source tarball, extracts it and copies the
 * test files according to the config file `cli/tests/node_compat/config.json`
 */

import { Foras, gunzip } from "https://deno.land/x/denoflate@2.0.2/deno/mod.ts";
import { Untar } from "../../test_util/std/archive/untar.ts";
import { walk } from "../../test_util/std/fs/walk.ts";
import {
  dirname,
  fromFileUrl,
  join,
  sep,
} from "../../test_util/std/path/mod.ts";
import { ensureFile } from "../../test_util/std/fs/ensure_file.ts";
import { Buffer } from "../../test_util/std/io/buffer.ts";
import { copy } from "../../test_util/std/streams/copy.ts";
import { readAll } from "../../test_util/std/streams/read_all.ts";
import { writeAll } from "../../test_util/std/streams/write_all.ts";
import { withoutAll } from "../../test_util/std/collections/without_all.ts";
import { relative } from "../../test_util/std/path/posix.ts";

import { config, ignoreList } from "../../cli/tests/node_compat/common.ts";

const encoder = new TextEncoder();

const NODE_VERSION = config.nodeVersion;
const NODE_NAME = "node-v" + NODE_VERSION;
const NODE_ARCHIVE_NAME = `${NODE_NAME}.tar.gz`;

const NODE_IGNORED_TEST_DIRS = [
  "addons",
  "async-hooks",
  "cctest",
  "common",
  "doctool",
  "embedding",
  "fixtures",
  "fuzzers",
  "js-native-api",
  "node-api",
  "overlapped-checker",
  "report",
  "testpy",
  "tick-processor",
  "tools",
  "v8-updates",
  "wasi",
  "wpt",
];

const NODE_TARBALL_URL =
  `https://nodejs.org/dist/v${NODE_VERSION}/${NODE_ARCHIVE_NAME}`;
const NODE_VERSIONS_ROOT = new URL("versions/", import.meta.url);
const NODE_TARBALL_LOCAL_URL = new URL(NODE_ARCHIVE_NAME, NODE_VERSIONS_ROOT);
// local dir url where we copy the node tests
const NODE_LOCAL_ROOT_URL = new URL(NODE_NAME, NODE_VERSIONS_ROOT);
const NODE_LOCAL_TEST_URL = new URL(NODE_NAME + "/test/", NODE_VERSIONS_ROOT);
const NODE_COMPAT_TEST_DEST_URL = new URL(
  "../../cli/tests/node_compat/test/",
  import.meta.url,
);

Foras.initSyncBundledOnce();

async function getNodeTests(): Promise<string[]> {
  const paths: string[] = [];
  const rootPath = NODE_LOCAL_TEST_URL.href.slice(7);
  for await (
    const item of walk(NODE_LOCAL_TEST_URL, { exts: [".js"] })
  ) {
    const path = relative(rootPath, item.path);
    if (NODE_IGNORED_TEST_DIRS.every((dir) => !path.startsWith(dir))) {
      paths.push(path);
    }
  }

  return paths.sort();
}

function getDenoTests() {
  return Object.entries(config.tests)
    .filter(([testDir]) => !NODE_IGNORED_TEST_DIRS.includes(testDir))
    .flatMap(([testDir, tests]) => tests.map((test) => testDir + "/" + test));
}

async function updateToDo() {
  const file = await Deno.open(new URL("./TODO.md", import.meta.url), {
    write: true,
    create: true,
    truncate: true,
  });

  const missingTests = withoutAll(await getNodeTests(), await getDenoTests());

  await file.write(encoder.encode(`<!-- deno-fmt-ignore-file -->
# Remaining Node Tests

NOTE: This file should not be manually edited. Please edit 'cli/tests/node_compat/config.json' and run 'tools/node_compat/setup.ts' instead.

Total: ${missingTests.length}

`));
  for (const test of missingTests) {
    await file.write(
      encoder.encode(
        `- [${test}](https://github.com/nodejs/node/tree/v${NODE_VERSION}/test/${test})\n`,
      ),
    );
  }
  file.close();
}

async function clearTests() {
  console.log("Cleaning up previous tests");
  for await (
    const file of walk(NODE_COMPAT_TEST_DEST_URL, {
      includeDirs: false,
      skip: ignoreList,
    })
  ) {
    await Deno.remove(file.path);
  }
}

async function decompressTests() {
  console.log(`Decompressing ${NODE_ARCHIVE_NAME}...`);

  const compressedFile = await Deno.open(NODE_TARBALL_LOCAL_URL);

  const buffer = new Buffer(gunzip(await readAll(compressedFile)));
  compressedFile.close();

  const tar = new Untar(buffer);
  const outFolder = dirname(fromFileUrl(NODE_TARBALL_LOCAL_URL));
  const testsFolder = `${NODE_NAME}/test`;

  for await (const entry of tar) {
    if (entry.type !== "file") continue;
    if (!entry.fileName.startsWith(testsFolder)) continue;
    const path = join(outFolder, entry.fileName);
    await ensureFile(path);
    const file = await Deno.open(path, {
      create: true,
      truncate: true,
      write: true,
    });
    await copy(entry, file);
    file.close();
  }
}

/** Checks if file has entry in config.json */
function hasEntry(file: string, suite: string) {
  return Array.isArray(config.tests[suite]) &&
    config.tests[suite].includes(file);
}

async function copyTests() {
  console.log("Copying test files...");

  for await (const entry of walk(NODE_LOCAL_TEST_URL, { skip: ignoreList })) {
    const fragments = entry.path.split(sep);
    // suite is the directory name after test/. For example, if the file is
    // "node-v18.12.1/test/fixtures/policy/main.mjs"
    // then suite is "fixtures/policy"
    const suite = fragments.slice(fragments.indexOf(NODE_NAME) + 2, -1)
      .join("/");
    if (!hasEntry(entry.name, suite)) {
      continue;
    }

    const dest = new URL(`${suite}/${entry.name}`, NODE_COMPAT_TEST_DEST_URL);
    await ensureFile(dest);
    const destFile = await Deno.open(dest, {
      create: true,
      truncate: true,
      write: true,
    });
    const srcFile = await Deno.open(
      new URL(`${suite}/${entry.name}`, NODE_LOCAL_TEST_URL),
    );
    await writeAll(
      destFile,
      encoder.encode(`// deno-fmt-ignore-file
// deno-lint-ignore-file

// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// Taken from Node ${NODE_VERSION}
// This file is automatically generated by "node/_tools/setup.ts". Do not modify this file manually

`),
    );
    await srcFile.readable.pipeTo(destFile.writable);
  }
}

/** Downloads Node tarball  */
async function downloadFile() {
  console.log(
    `Downloading ${NODE_TARBALL_URL} in "${NODE_TARBALL_LOCAL_URL}" ...`,
  );
  const response = await fetch(NODE_TARBALL_URL);
  if (!response.ok) {
    throw new Error(`Request failed with status ${response.status}`);
  }
  await ensureFile(NODE_TARBALL_LOCAL_URL);
  const file = await Deno.open(NODE_TARBALL_LOCAL_URL, {
    truncate: true,
    write: true,
    create: true,
  });
  await response.body.pipeTo(file.writable);
}

// main

try {
  Deno.lstatSync(NODE_TARBALL_LOCAL_URL);
} catch (e) {
  if (!(e instanceof Deno.errors.NotFound)) {
    throw e;
  }
  await downloadFile();
}

try {
  Deno.lstatSync(NODE_LOCAL_ROOT_URL);
} catch (e) {
  if (!(e instanceof Deno.errors.NotFound)) {
    throw e;
  }
  await decompressTests();
}

await clearTests();
await copyTests();
await updateToDo();
