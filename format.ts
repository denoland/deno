#!/usr/bin/env deno --allow-run --allow-write
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/**
 * This script formats the source files in the repository.
 *
 * Usage: deno format.ts [--check]
 *
 * Options:
 *   --check          Checks if the source files are formatted.
 */
import { args, platform, readAll, exit, run, readFile, writeFile } from "deno";
import { parse } from "./flags/mod.ts";
import { prettier, prettierPlugins } from "./prettier/prettier.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// Runs commands in cross-platform way
function xrun(opts) {
  return run({
    ...opts,
    args: platform.os === "win" ? ["cmd.exe", "/c", ...opts.args] : opts.args
  });
}

// Gets the source files in the repository
async function getSourceFiles() {
  return decoder
    .decode(
      await readAll(
        xrun({
          args: ["git", "ls-files"],
          stdout: "piped"
        }).stdout
      )
    )
    .trim()
    .split(/\r?\n/);
}

async function readFileIfExists(filename: string): Promise<string | null> {
  let data;
  try {
    data = await readFile(filename);
  } catch (e) {
    // The file is deleted. Returns null.
    return null;
  }

  return decoder.decode(data);
}

/**
 * Checks if the file has been formatted with prettier.
 */
async function checkFile(
  filename: string,
  parser: "typescript" | "markdown"
): Promise<boolean> {
  const text = await readFileIfExists(filename);

  if (!text) {
    // The file is deleted. Skip.
    return;
  }

  const formatted = prettier.check(text, {
    parser,
    plugins: prettierPlugins
  });

  if (!formatted) {
    // TODO: print some diff info here to show why this failed
    console.error(`${filename} ... Not formatted`);
  }

  return formatted;
}

/**
 * Formats the given file.
 */
async function formatFile(
  filename: string,
  parser: "typescript" | "markdown"
): Promise<void> {
  const text = await readFileIfExists(filename);

  if (!text) {
    // The file is deleted. Skip.
    return;
  }

  const formatted = prettier.format(text, {
    parser,
    plugins: prettierPlugins
  });

  if (text !== formatted) {
    console.log(`Formatting ${filename}`);
    await writeFile(filename, encoder.encode(formatted));
  }
}

/**
 * Checks if the all files have been formatted with prettier.
 */
async function checkSourceFiles() {
  const checks = [];

  (await getSourceFiles()).forEach(file => {
    if (/\.ts$/.test(file)) {
      checks.push(checkFile(file, "typescript"));
    } else if (/\.md$/.test(file)) {
      checks.push(checkFile(file, "markdown"));
    }
  });

  const results = await Promise.all(checks);

  if (results.every(result => result)) {
    exit(0);
  } else {
    exit(1);
  }
}

/**
 * Formats the all files with prettier.
 */
async function formatSourceFiles() {
  const formats = [];

  (await getSourceFiles()).forEach(file => {
    if (/\.ts$/.test(file)) {
      formats.push(formatFile(file, "typescript"));
    } else if (/\.md$/.test(file)) {
      formats.push(formatFile(file, "markdown"));
    }
  });

  await Promise.all(formats);
  exit(0);
}

async function main(opts) {
  try {
    if (opts.check) {
      await checkSourceFiles();
    } else {
      await formatSourceFiles();
    }
  } catch (e) {
    console.log(e);
    exit(1);
  }
}

main(parse(args));
