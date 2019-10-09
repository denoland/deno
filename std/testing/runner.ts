#!/usr/bin/env -S deno -A
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { parse } from "../flags/mod.ts";
import { ExpandGlobOptions, expandGlob } from "../fs/mod.ts";
import { isWindows } from "../fs/path/constants.ts";
import { join } from "../fs/path/mod.ts";
import { RunTestsOptions, runTests } from "./mod.ts";
const { DenoError, ErrorKind, args, cwd, exit } = Deno;

const DIR_GLOBS = [join("**", "?(*_)test.{js,ts}")];

function showHelp(): void {
  console.log(`Deno test runner

USAGE:
  deno -A https://deno.land/std/testing/runner.ts [OPTIONS] [MODULES...]

OPTIONS:
  -q, --quiet                 Don't show output from test cases
  -f, --failfast              Stop running tests on first error
  -e, --exclude <MODULES...>  List of comma-separated modules to exclude
  --allow-none                Exit with status 0 even when no test modules are
                              found

ARGS:
  [MODULES...]  List of test modules to run.
                A directory <dir> will expand to:
                  ${DIR_GLOBS.map((s: string): string => `${join("<dir>", s)}`)
                    .join(`
                  `)}
                Defaults to "." when none are provided.

Note that modules can refer to file paths or URLs. File paths support glob
expansion.

Examples:
      deno test src/**/*_test.ts
      deno test tests`);
}

function isRemoteUrl(url: string): boolean {
  return /^https?:\/\//.test(url);
}

function partition(
  arr: string[],
  callback: (el: string) => boolean
): [string[], string[]] {
  return arr.reduce(
    (paritioned: [string[], string[]], el: string): [string[], string[]] => {
      paritioned[callback(el) ? 1 : 0].push(el);
      return paritioned;
    },
    [[], []]
  );
}

function filePathToUrl(path: string): string {
  return `file://${isWindows ? "/" : ""}${path.replace(/\\/g, "/")}`;
}

/**
 * Given a list of globs or URLs to include and exclude and a root directory
 * from which to expand relative globs, yield a list of URLs
 * (file: or remote) that should be imported for the test runner.
 */
export async function* findTestModules(
  includeModules: string[],
  excludeModules: string[],
  root: string = cwd()
): AsyncIterableIterator<string> {
  const [includePaths, includeUrls] = partition(includeModules, isRemoteUrl);
  const [excludePaths, excludeUrls] = partition(excludeModules, isRemoteUrl);

  const expandGlobOpts: ExpandGlobOptions = {
    root,
    exclude: excludePaths,
    includeDirs: true,
    extended: true,
    globstar: true
  };

  async function* expandDirectory(d: string): AsyncIterableIterator<string> {
    for (const dirGlob of DIR_GLOBS) {
      for await (const walkInfo of expandGlob(dirGlob, {
        ...expandGlobOpts,
        root: d,
        includeDirs: false
      })) {
        yield filePathToUrl(walkInfo.filename);
      }
    }
  }

  for (const globString of includePaths) {
    for await (const walkInfo of expandGlob(globString, expandGlobOpts)) {
      if (walkInfo.info.isDirectory()) {
        yield* expandDirectory(walkInfo.filename);
      } else {
        yield filePathToUrl(walkInfo.filename);
      }
    }
  }

  const excludeUrlPatterns = excludeUrls.map(
    (url: string): RegExp => RegExp(url)
  );
  const shouldIncludeUrl = (url: string): boolean =>
    !excludeUrlPatterns.some((p: RegExp): boolean => !!url.match(p));

  yield* includeUrls.filter(shouldIncludeUrl);
}

export interface RunTestModulesOptions extends RunTestsOptions {
  include?: string[];
  exclude?: string[];
  allowNone?: boolean;
}

/**
 * Import the specified test modules and run their tests as a suite.
 *
 * Test modules are specified as an array of strings and can include local files
 * or URLs.
 *
 * File matching and excluding support glob syntax - arguments recognized as
 * globs will be expanded using `glob()` from the `fs` module.
 *
 * Example:
 *
 *       runTestModules({ include: ["**\/*_test.ts", "**\/test.ts"] });
 *
 * Any matched directory `<dir>` will expand to:
 *   <dir>/**\/?(*_)test.{js,ts}
 *
 * So the above example is captured naturally by:
 *
 *       runTestModules({ include: ["."] });
 *
 * Which is the default used for:
 *
 *       runTestModules();
 */
// TODO: Change return type to `Promise<void>` once, `runTests` is updated
// to return boolean instead of exiting.
export async function runTestModules({
  include = ["."],
  exclude = [],
  allowNone = false,
  parallel = false,
  exitOnFail = false,
  only = /[^\s]/,
  skip = /^\s*$/,
  disableLog = false
}: RunTestModulesOptions = {}): Promise<void> {
  let moduleCount = 0;
  for await (const testModule of findTestModules(include, exclude)) {
    await import(testModule);
    moduleCount++;
  }

  if (moduleCount == 0) {
    const noneFoundMessage = "No matching test modules found.";
    if (!allowNone) {
      throw new DenoError(ErrorKind.NotFound, noneFoundMessage);
    } else if (!disableLog) {
      console.log(noneFoundMessage);
    }
    return;
  }

  if (!disableLog) {
    console.log(`Found ${moduleCount} matching test modules.`);
  }

  await runTests({
    parallel,
    exitOnFail,
    only,
    skip,
    disableLog
  });
}

async function main(): Promise<void> {
  const parsedArgs = parse(args.slice(1), {
    boolean: ["allow-none", "failfast", "help", "quiet"],
    string: ["exclude"],
    alias: {
      exclude: ["e"],
      failfast: ["f"],
      help: ["h"],
      quiet: ["q"]
    },
    default: {
      "allow-none": false,
      failfast: false,
      help: false,
      quiet: false
    }
  });
  if (parsedArgs.help) {
    return showHelp();
  }

  const include =
    parsedArgs._.length > 0
      ? (parsedArgs._ as string[]).flatMap((fileGlob: string): string[] =>
          fileGlob.split(",")
        )
      : ["."];
  const exclude =
    parsedArgs.exclude != null ? (parsedArgs.exclude as string).split(",") : [];
  const allowNone = parsedArgs["allow-none"];
  const exitOnFail = parsedArgs.failfast;
  const disableLog = parsedArgs.quiet;

  try {
    await runTestModules({
      include,
      exclude,
      allowNone,
      exitOnFail,
      disableLog
    });
  } catch (error) {
    if (!disableLog) {
      console.error(error.message);
    }
    exit(1);
  }
}

if (import.meta.main) {
  main();
}
