#!/usr/bin/env -S deno -A
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { parse } from "../flags/mod.ts";
import { glob, isGlob, walk } from "../fs/mod.ts";
import { runTests } from "./mod.ts";
const { args, cwd } = Deno;

const DEFAULT_GLOBS = [
  "**/*_test.ts",
  "**/*_test.js",
  "**/test.ts",
  "**/test.js"
];

/* eslint-disable max-len */
function showHelp(): void {
  console.log(`Deno test runner

USAGE:
  deno -A https://deno.land/std/testing/runner.ts [OPTIONS] [FILES...]

OPTIONS:
  -q, --quiet               Don't show output from test cases 
  -f, --failfast            Stop test suite on first error
  -e, --exclude <FILES...>  List of file names to exclude from run. If this options is 
                            used files to match must be specified after "--". 
  
ARGS:
  [FILES...]  List of file names to run. Defaults to: ${DEFAULT_GLOBS.join(
    ","
  )} 
`);
}
/* eslint-enable max-len */

function filePathToRegExp(str: string): RegExp {
  if (isGlob(str)) {
    return glob(str, { flags: "g" });
  }

  return RegExp(str, "g");
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

/**
 * Given list of globs or URLs to include and exclude and root directory return
 * list of file URLs that should be imported for test runner.
 */
export async function getMatchingUrls(
  matchPaths: string[],
  excludePaths: string[],
  root: string
): Promise<string[]> {
  const [includeLocal, includeRemote] = partition(matchPaths, isRemoteUrl);
  const [excludeLocal, excludeRemote] = partition(excludePaths, isRemoteUrl);

  const localFileIterator = walk(root, {
    match: includeLocal.map((f: string): RegExp => filePathToRegExp(f)),
    skip: excludeLocal.map((f: string): RegExp => filePathToRegExp(f))
  });

  let matchingLocalUrls: string[] = [];
  for await (const { filename } of localFileIterator) {
    matchingLocalUrls.push(`file://${filename}`);
  }

  const excludeRemotePatterns = excludeRemote.map(
    (url: string): RegExp => RegExp(url)
  );
  const matchingRemoteUrls = includeRemote.filter(
    (candidateUrl: string): boolean => {
      return !excludeRemotePatterns.some((pattern: RegExp): boolean => {
        const r = pattern.test(candidateUrl);
        pattern.lastIndex = 0;
        return r;
      });
    }
  );

  return matchingLocalUrls.concat(matchingRemoteUrls);
}
/**
 * This function runs matching test files in `root` directory.
 *
 * File matching and excluding supports glob syntax, ie. if encountered arg is
 * a glob it will be expanded using `glob` method from `fs` module.
 *
 * Note that your shell may expand globs for you:
 *    $ deno -A ./runner.ts **\/*_test.ts **\/test.ts
 *
 * Expanding using `fs.glob`:
 *    $ deno -A ./runner.ts \*\*\/\*_test.ts \*\*\/test.ts
 *
 *  `**\/*_test.ts` and `**\/test.ts"` are arguments that will be parsed and
 *  expanded as: [glob("**\/*_test.ts"), glob("**\/test.ts")]
 */
// TODO: change return type to `Promise<void>` once, `runTests` is updated
// to return boolean instead of exiting
export async function main(root: string = cwd()): Promise<void> {
  const parsedArgs = parse(args.slice(1), {
    boolean: ["quiet", "failfast", "help"],
    string: ["exclude"],
    alias: {
      help: ["h"],
      quiet: ["q"],
      failfast: ["f"],
      exclude: ["e"]
    }
  });

  if (parsedArgs.help) {
    return showHelp();
  }

  let includeFiles: string[];
  let excludeFiles: string[];

  if (parsedArgs._.length) {
    includeFiles = (parsedArgs._ as string[])
      .map((fileGlob: string): string[] => {
        return fileGlob.split(",");
      })
      .flat();
  } else {
    includeFiles = DEFAULT_GLOBS;
  }

  if (parsedArgs.exclude) {
    excludeFiles = (parsedArgs.exclude as string).split(",");
  } else {
    excludeFiles = [];
  }

  const foundTestUrls = await getMatchingUrls(includeFiles, excludeFiles, root);

  if (foundTestUrls.length === 0) {
    console.error("No matching test files found.");
    return;
  }

  console.log(`Found ${foundTestUrls.length} matching test files.`);

  for (const url of foundTestUrls) {
    await import(url);
  }

  await runTests({
    exitOnFail: !!parsedArgs.failfast,
    disableLog: !!parsedArgs.quiet
  });
}

if (import.meta.main) {
  main();
}
