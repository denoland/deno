#!/usr/bin/env deno --allow-run --allow-write
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// This script formats the given source files. If the files are omitted, it
// formats the all files in the repository.
const { args, exit, readFile, writeFile } = Deno;
type FileInfo = Deno.FileInfo;
import { glob } from "../fs/glob.ts";
import { walk } from "../fs/walk.ts";
import { parse } from "../flags/mod.ts";
import { prettier, prettierPlugins } from "./prettier.ts";

const HELP_MESSAGE = `
Formats the given files. If no arg is passed, then formats the all files.

Usage: deno prettier/main.ts [options] [files...]

Options:
  -H, --help                 Show this help message and exit.
  --check                    Check if the source files are formatted.
  --ignore <path>            Ignore the given path(s).

Example:
  deno prettier/main.ts script1.ts script2.js
                             Formats the files

  deno prettier/main.ts --check script1.ts script2.js
                             Checks if the files are formatted

  deno prettier/main.ts
                             Formats the all files in the repository
`;

// Available parsers
type ParserLabel = "typescript" | "babel" | "markdown" | "json";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

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
  parser: ParserLabel
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
  parser: ParserLabel
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
 * Selects the right prettier parser for the given path.
 */
function selectParser(path: string): ParserLabel | null {
  if (/\.ts$/.test(path)) {
    return "typescript";
  } else if (/\.js$/.test(path)) {
    return "babel";
  } else if (/\.json$/.test(path)) {
    return "json";
  } else if (/\.md$/.test(path)) {
    return "markdown";
  }

  return null;
}

/**
 * Checks if the files of the given paths have been formatted with prettier.
 * If paths are empty, then checks all the files.
 */
async function checkSourceFiles(
  files: AsyncIterableIterator<FileInfo>
): Promise<void> {
  const checks: Array<Promise<boolean>> = [];

  for await (const file of files) {
    const parser = selectParser(file.path);
    if (parser) {
      checks.push(checkFile(file.path, parser));
    }
  }

  const results = await Promise.all(checks);

  if (results.every(result => result)) {
    console.log("Every file is formatted");
    exit(0);
  } else {
    console.log("Some files are not formatted");
    exit(1);
  }
}

/**
 * Formats the files of the given paths with prettier.
 * If paths are empty, then formats all the files.
 */
async function formatSourceFiles(
  files: AsyncIterableIterator<FileInfo>
): Promise<void> {
  const formats: Array<Promise<void>> = [];

  for await (const file of files) {
    const parser = selectParser(file.path);
    if (parser) {
      formats.push(formatFile(file.path, parser));
    }
  }

  await Promise.all(formats);
  exit(0);
}

async function main(opts): Promise<void> {
  const { help, ignore, check, _: args } = opts;

  if (help) {
    console.log(HELP_MESSAGE);
    exit(0);
  }
  const options = { flags: "g" };
  const skip = Array.isArray(ignore)
    ? ignore.map((i: string) => glob(i, options))
    : [glob(ignore, options)];
  const match =
    args.length > 0 ? args.map((a: string) => glob(a, options)) : undefined;
  const files = walk(".", { match, skip });
  try {
    if (check) {
      await checkSourceFiles(files);
    } else {
      await formatSourceFiles(files);
    }
  } catch (e) {
    console.log(e);
    exit(1);
  }
}

main(
  parse(args.slice(1), {
    string: ["ignore"],
    boolean: ["check", "help"],
    default: {
      ignore: []
    },
    alias: {
      H: "help"
    }
  })
);
