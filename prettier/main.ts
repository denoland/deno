#!/usr/bin/env deno --allow-run --allow-write
/**
 * Copyright © James Long and contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
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

JS/TS Styling Options:
  --print-width <int>        The line length where Prettier will try wrap.
                             Defaults to 80.
  --tab-width <int>          Number of spaces per indentation level.
                             Defaults to 2.
  --use-tabs                 Indent with tabs instead of spaces.
                             Defaults to false.
  --no-semi                  Do not print semicolons, except at the beginning of lines which may need them.
  --single-quote             Use single quotes instead of double quotes.
                             Defaults to false.
  --trailing-comma <none|es5|all>
                             Print trailing commas wherever possible when multi-line.
                             Defaults to none.
  --no-bracket-spacing       Do not print spaces between brackets.
  --arrow-parens <avoid|always>
                             Include parentheses around a sole arrow function parameter.
                             Defaults to avoid.
  --end-of-line <auto|lf|crlf|cr>
                             Which end of line characters to apply.
                             Defaults to auto.

Markdown Styling Options:
  --prose-wrap <always|never|preserve>
                             How to wrap prose.
                             Defaults to preserve.

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

interface PrettierOptions {
  printWidth: number;
  tabWidth: number;
  useTabs: boolean;
  semi: boolean;
  singleQuote: boolean;
  trailingComma: string;
  bracketSpacing: boolean;
  arrowParens: string;
  proseWrap: string;
  endOfLine: string;
}

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
  parser: ParserLabel,
  prettierOpts: PrettierOptions
): Promise<boolean> {
  const text = await readFileIfExists(filename);

  if (!text) {
    // The file is empty. Skip.
    return true;
  }

  const formatted = prettier.check(text, {
    ...prettierOpts,
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
  parser: ParserLabel,
  prettierOpts: PrettierOptions
): Promise<void> {
  const text = await readFileIfExists(filename);

  if (!text) {
    // The file is deleted. Skip.
    return;
  }

  const formatted = prettier.format(text, {
    ...prettierOpts,
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
  files: AsyncIterableIterator<FileInfo>,
  prettierOpts: PrettierOptions
): Promise<void> {
  const checks: Array<Promise<boolean>> = [];

  for await (const file of files) {
    const parser = selectParser(file.path);
    if (parser) {
      checks.push(checkFile(file.path, parser, prettierOpts));
    }
  }

  const results = await Promise.all(checks);

  if (results.every((result): boolean => result)) {
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
  files: AsyncIterableIterator<FileInfo>,
  prettierOpts: PrettierOptions
): Promise<void> {
  const formats: Array<Promise<void>> = [];

  for await (const file of files) {
    const parser = selectParser(file.path);
    if (parser) {
      formats.push(formatFile(file.path, parser, prettierOpts));
    }
  }

  await Promise.all(formats);
  exit(0);
}

async function main(opts): Promise<void> {
  const { help, ignore, check, _: args } = opts;

  const prettierOpts: PrettierOptions = {
    printWidth: Number(opts["print-width"]),
    tabWidth: Number(opts["tab-width"]),
    useTabs: Boolean(opts["use-tabs"]),
    semi: Boolean(opts["semi"]),
    singleQuote: Boolean(opts["single-quote"]),
    trailingComma: opts["trailing-comma"],
    bracketSpacing: Boolean(opts["bracket-spacing"]),
    arrowParens: opts["arrow-parens"],
    proseWrap: opts["prose-wrap"],
    endOfLine: opts["end-of-line"]
  };

  if (help) {
    console.log(HELP_MESSAGE);
    exit(0);
  }
  const options = { flags: "g" };
  const skip = Array.isArray(ignore)
    ? ignore.map((i: string): RegExp => glob(i, options))
    : [glob(ignore, options)];
  const match =
    args.length > 0
      ? args.map((a: string): RegExp => glob(a, options))
      : undefined;
  const files = walk(".", { match, skip });
  try {
    if (check) {
      await checkSourceFiles(files, prettierOpts);
    } else {
      await formatSourceFiles(files, prettierOpts);
    }
  } catch (e) {
    console.log(e);
    exit(1);
  }
}

main(
  parse(args.slice(1), {
    string: [
      "ignore",
      "printWidth",
      "tab-width",
      "trailing-comma",
      "arrow-parens",
      "prose-wrap",
      "end-of-line"
    ],
    boolean: [
      "check",
      "help",
      "semi",
      "use-tabs",
      "single-quote",
      "bracket-spacing"
    ],
    default: {
      ignore: [],
      "print-width": "80",
      "tab-width": "2",
      "use-tabs": false,
      semi: true,
      "single-quote": false,
      "trailing-comma": "none",
      "bracket-spacing": true,
      "arrow-parens": "avoid",
      "prose-wrap": "preserve",
      "end-of-line": "auto"
    },
    alias: {
      H: "help"
    }
  })
);
