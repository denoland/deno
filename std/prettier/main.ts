#!/usr/bin/env -S deno --allow-run --allow-write
/**
 * Copyright © James Long and contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// This script formats the given source files. If the files are omitted, it
// formats the all files in the repository.
import { parse } from "../flags/mod.ts";
import { ExpandGlobOptions, WalkInfo, expandGlob } from "../fs/mod.ts";
import { prettier, prettierPlugins } from "./prettier.ts";
const { args, cwd, exit, readAll, readFile, stdin, stdout, writeFile } = Deno;

const HELP_MESSAGE = `
Formats the given files. If no arg is passed, then formats the all files.

Usage: deno prettier/main.ts [options] [files...]

Options:
  -H, --help                            Show this help message and exit.
  --check                               Check if the source files are formatted.
  --write                               Whether to write to the file, otherwise
                                        it will output to stdout, Defaults to
                                        false.
  --ignore <path>                       Ignore the given path(s).
  --stdin                               Specifies to read the code from stdin.
                                        If run the command in a pipe, you do not
                                        need to specify this flag.
                                        Defaults to false.
  --stdin-parser <typescript|babel|markdown|json>
                                        If set --stdin flag, then need specify a
                                        parser for stdin. available parser:
                                        typescript/babel/markdown/json. Defaults
                                        to typescript.

JS/TS Styling Options:
  --print-width <int>                   The line length where Prettier will try
                                        wrap. Defaults to 80.
  --tab-width <int>                     Number of spaces per indentation level.
                                        Defaults to 2.
  --use-tabs                            Indent with tabs instead of spaces.
                                        Defaults to false.
  --no-semi                             Do not print semicolons, except at the
                                        beginning of lines which may need them.
  --single-quote                        Use single quotes instead of double
                                        quotes. Defaults to false.
  --trailing-comma <none|es5|all>       Print trailing commas wherever possible
                                        when multi-line. Defaults to none.
  --no-bracket-spacing                  Do not print spaces between brackets.
  --arrow-parens <avoid|always>         Include parentheses around a sole arrow
                                        function parameter. Defaults to avoid.
  --end-of-line <auto|lf|crlf|cr>       Which end of line characters to apply.
                                        Defaults to auto.

Markdown Styling Options:
  --prose-wrap <always|never|preserve>  How to wrap prose. Defaults to preserve.

Example:
  deno run prettier/main.ts --write script1.ts script2.js
                                        Formats the files

  deno run prettier/main.ts --check script1.ts script2.js
                                        Checks if the files are formatted

  deno run prettier/main.ts --write
                                        Formats the all files in the repository

  deno run prettier/main.ts script1.ts
                                        Print the formatted code to stdout

  cat script1.ts | deno run prettier/main.ts
                                        Read the typescript code from stdin and
                                        output formatted code to stdout.

  cat config.json | deno run prettier/main.ts --stdin-parser=json
                                        Read the JSON string from stdin and
                                        output formatted code to stdout.
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
  write: boolean;
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

  const formatted: string = prettier.format(text, {
    ...prettierOpts,
    parser,
    plugins: prettierPlugins
  });

  const fileUnit8 = encoder.encode(formatted);
  if (prettierOpts.write) {
    if (text !== formatted) {
      console.log(`Formatting ${filename}`);
      await writeFile(filename, fileUnit8);
    }
  } else {
    await stdout.write(fileUnit8);
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
  files: AsyncIterableIterator<WalkInfo>,
  prettierOpts: PrettierOptions
): Promise<void> {
  const checks: Array<Promise<boolean>> = [];

  for await (const { filename } of files) {
    const parser = selectParser(filename);
    if (parser) {
      checks.push(checkFile(filename, parser, prettierOpts));
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
  files: AsyncIterableIterator<WalkInfo>,
  prettierOpts: PrettierOptions
): Promise<void> {
  const formats: Array<Promise<void>> = [];

  for await (const { filename } of files) {
    const parser = selectParser(filename);
    if (parser) {
      formats.push(formatFile(filename, parser, prettierOpts));
    }
  }

  await Promise.all(formats);
  exit(0);
}

/**
 * Format source code
 */
function format(
  text: string,
  parser: ParserLabel,
  prettierOpts: PrettierOptions
): string {
  const formatted: string = prettier.format(text, {
    ...prettierOpts,
    parser: parser,
    plugins: prettierPlugins
  });

  return formatted;
}

/**
 * Format code from stdin and output to stdout
 */
async function formatFromStdin(
  parser: ParserLabel,
  prettierOpts: PrettierOptions
): Promise<void> {
  const byte = await readAll(stdin);
  const formattedCode = format(
    new TextDecoder().decode(byte),
    parser,
    prettierOpts
  );
  await stdout.write(new TextEncoder().encode(formattedCode));
}

/**
 * Get the files to format.
 * @param include The glob patterns to select the files.
 *                  eg `"cmd/*.ts"` to select all the typescript files in cmd
 *                  directory.
 *                  eg `"cmd/run.ts"` to select `cmd/run.ts` file as only.
 * @param exclude The glob patterns to ignore files.
 *                  eg `"*_test.ts"` to ignore all the test file.
 * @param root    The directory from which to apply default globs.
 * @returns returns an async iterable object
 */
async function* getTargetFiles(
  include: string[],
  exclude: string[],
  root: string = cwd()
): AsyncIterableIterator<WalkInfo> {
  const expandGlobOpts: ExpandGlobOptions = {
    root,
    exclude,
    includeDirs: true,
    extended: true,
    globstar: true
  };

  async function* expandDirectory(d: string): AsyncIterableIterator<WalkInfo> {
    for await (const walkInfo of expandGlob("**/*", {
      ...expandGlobOpts,
      root: d,
      includeDirs: false
    })) {
      yield walkInfo;
    }
  }

  for (const globString of include) {
    for await (const walkInfo of expandGlob(globString, expandGlobOpts)) {
      if (walkInfo.info.isDirectory()) {
        yield* expandDirectory(walkInfo.filename);
      } else {
        yield walkInfo;
      }
    }
  }
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
    endOfLine: opts["end-of-line"],
    write: opts["write"]
  };

  if (help) {
    console.log(HELP_MESSAGE);
    exit(0);
  }

  const files = getTargetFiles(
    args.length ? args : ["."],
    Array.isArray(ignore) ? ignore : [ignore]
  );

  const tty = Deno.isTTY();

  const shouldReadFromStdin =
    (!tty.stdin && (tty.stdout || tty.stderr)) || !!opts["stdin"];

  try {
    if (shouldReadFromStdin) {
      await formatFromStdin(opts["stdin-parser"], prettierOpts);
    } else if (check) {
      await checkSourceFiles(files, prettierOpts);
    } else {
      await formatSourceFiles(files, prettierOpts);
    }
  } catch (e) {
    console.error(e);
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
      "end-of-line",
      "stdin-parser"
    ],
    boolean: [
      "check",
      "help",
      "semi",
      "use-tabs",
      "single-quote",
      "bracket-spacing",
      "write",
      "stdin"
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
      "end-of-line": "auto",
      write: false,
      stdin: false,
      "stdin-parser": "typescript"
    },
    alias: {
      H: "help"
    }
  })
);
