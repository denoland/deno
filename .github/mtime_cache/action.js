// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// This file contains the implementation of a Github Action. Github uses
// Node.js v12.x to run actions, so this is Node code and not Deno code.

const { spawn } = require("child_process");
const { dirname, resolve } = require("path");
const { StringDecoder } = require("string_decoder");
const { promisify } = require("util");

const fs = require("fs");
const utimes = promisify(fs.utimes);
const mkdir = promisify(fs.mkdir);
const readFile = promisify(fs.readFile);
const writeFile = promisify(fs.writeFile);

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
    ["-C", dir || ".", "ls-files", "--stage", "--eol", "--full-name", "-z"],
    { stdio: ["ignore", "pipe", "inherit"] },
  );
  output = decode(output, "utf8");
  output = split(output, "\0");
  output = filter(output, Boolean);

  for await (const entry of output) {
    const pat =
      /^(?<mode>\d{6}) (?<hash>[0-9a-f]{40}) 0\t(?<eol>[^\t]*?)[ ]*\t(?<name>.*)$/;
    const { mode, hash, eol, name } = pat.exec(entry).groups;
    const path = dir ? `${dir}/${name}` : name;

    switch (mode) {
      case "120000": // Symbolic link.
        break;
      case "160000": // Git submodule.
        yield* ls(path);
        break;
      default: {
        // Regular file.
        const key = [mode, hash, eol, path].join("\0");
        yield { key, path };
      }
    }
  }
}

async function* run(cmd, args, options) {
  const child = spawn(cmd, args, options);

  const promise = new Promise((resolve, reject) => {
    child.on("close", (code, signal) => {
      if (code === 0 && signal === null) {
        resolve();
      } else {
        const command = [cmd, ...args].join(" ");
        const how = signal === null ? `exit code ${code}` : `signal ${signal}`;
        const error = new Error(`Command '${command}' failed: ${how}`);
        reject(error);
      }
    });
    child.on("error", reject);
  });

  yield* child.stdout;
  await promise;
}

async function collect(stream) {
  const array = [];
  for await (const item of stream) {
    array.push(item);
  }
  return array;
}

async function* decode(stream, encoding) {
  const decoder = new StringDecoder(encoding);
  for await (const chunk of stream) {
    yield decoder.write(chunk);
  }
  yield decoder.end();
}

async function* filter(stream, fn) {
  for await (const item of stream) {
    if (fn(item)) yield item;
  }
}

async function* split(stream, separator) {
  let buf = "";
  for await (const chunk of stream) {
    const parts = (buf + chunk).split(separator);
    buf = parts.pop();
    yield* parts.values();
  }
  yield buf;
}
