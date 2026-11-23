// Copyright 2018-2026 the Deno authors. MIT license.
// This file contains the implementation of a Github Action. Github uses
// Node.js v20.x to run actions, so this is Node code and not Deno code.

const { spawn } = require("child_process");
const { existsSync } = require("fs");
const { utimes, mkdir, readFile, writeFile } = require("fs/promises");
const { dirname, resolve } = require("path");
const { StringDecoder } = require("string_decoder");

process.on("unhandledRejection", abort);
main().catch(abort);

async function main() {
  const startTime = getTime();

  const checkCleanPromise = checkClean();

  const cacheFile = getCacheFile();
  const oldCache = await loadCache(cacheFile);
  const newCache = Object.create(null);

  await checkCleanPromise;

  const counters = {
    restored: 0,
    added: 0,
    stale: 0,
    invalid: 0,
  };

  for await (const { key, path } of ls()) {
    let mtime = oldCache[key];
    if (mtime === undefined) {
      mtime = startTime;
      counters.added++;
    } else if (!mtime || mtime > startTime) {
      mtime = startTime;
      counters.invalid++;
    } else {
      counters.restored++;
    }

    await utimes(path, startTime, mtime);
    newCache[key] = mtime;
  }

  for (const key of Object.keys(oldCache)) {
    if (!(key in newCache)) counters.stale++;
  }

  await saveCache(cacheFile, newCache);

  const stats = {
    ...counters,
    "cache file": cacheFile,
    "time spent": (getTime() - startTime).toFixed(3) + "s",
  };
  console.log(
    [
      "mtime cache statistics",
      ...Object.entries(stats).map(([k, v]) => `* ${k}: ${v}`),
    ].join("\n"),
  );
}

function abort(err) {
  console.error(err);
  process.exit(1);
}

function getTime() {
  return Date.now() / 1000;
}

function getCacheFile() {
  const cachePath = process.env["INPUT_CACHE-PATH"];
  if (cachePath == null) {
    throw new Error("required input 'cache_path' not provided");
  }

  const cacheFile = resolve(cachePath, ".mtime-cache-db.json");
  return cacheFile;
}

async function loadCache(cacheFile) {
  try {
    const json = await readFile(cacheFile, { encoding: "utf8" });
    return JSON.parse(json);
  } catch (err) {
    if (err.code !== "ENOENT") {
      console.warn(`failed to load mtime cache from '${cacheFile}': ${err}`);
    }
    return Object.create(null);
  }
}

async function saveCache(cacheFile, cacheData) {
  const cacheDir = dirname(cacheFile);
  await mkdir(cacheDir, { recursive: true });

  const json = JSON.stringify(cacheData, null, 2);
  await writeFile(cacheFile, json, { encoding: "utf8" });
}

async function checkClean() {
  let output = run(
    "git",
    [
      "status",
      "--porcelain=v1",
      "--ignore-submodules=untracked",
      "--untracked-files=no",
    ],
    { stdio: ["ignore", "pipe", "inherit"] },
  );
  output = decode(output, "utf8");
  output = split(output, "\n");
  output = filter(output, Boolean);
  output = await collect(output);

  if (output.length > 0) {
    throw new Error(
      ["git work dir dirty", ...output.map((f) => `  ${f}`)].join("\n"),
    );
  }
}

async function* ls(dir = "") {
  let output = run(
    "git",
    ["-C", dir || ".", "ls-files", "--stage", "--e"],
    { stdio: ["ignore", "pipe", "inherit"] },
  );
  output = decode(output, "utf8");
  output = split(output, "\n");
  output = filter(output, Boolean);
  for await (const line of output) {
    const parts = line.split("\t");
    if (parts.length !== 2) continue;
    const [meta, path] = parts;
    const metaParts = meta.split(" ");
    if (metaParts.length < 3) continue;
    const key = metaParts[1];
    yield { key, path: resolve(dir, path) };
  }
}

function run(command, args, options) {
  const child = spawn(command, args, options);
  return child.stdout;
}

async function* collect(iterable) {
  for await (const item of iterable) {
    yield item;
  }
}

function decode(stream, encoding) {
  const decoder = new StringDecoder(encoding);
  return (async function* () {
    for await (const chunk of stream) {
      yield decoder.write(chunk);
    }
    const tail = decoder.end();
    if (tail) {
      yield tail;
    }
  })();
}

function split(iterable, delimiter) {
  return (async function* () {
    let buffer = "";
    for await (const chunk of iterable) {
      buffer += chunk;
      let idx;
      while ((idx = buffer.indexOf(delimiter)) !== -1) {
        yield buffer.slice(0, idx);
        buffer = buffer.slice(idx + delimiter.length);
      }
    }
    if (buffer.length > 0) {
      yield buffer;
    }
  })();
}

function filter(iterable, predicate) {
  return (async function* () {
    for await (const item of iterable) {
      if (predicate(item)) {
        yield item;
      }
    }
  })();
}