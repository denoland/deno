#!/usr/bin/env -S deno -A
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { parse } from "../flags/mod.ts";
import { ExpandGlobOptions, expandGlob } from "../fs/mod.ts";
import { isWindows, join } from "../path/mod.ts";
const { args, cwd, exit } = Deno;

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
    globstar: true,
  };

  async function* expandDirectory(d: string): AsyncIterableIterator<string> {
    for (const dirGlob of DIR_GLOBS) {
      for await (const walkInfo of expandGlob(dirGlob, {
        ...expandGlobOpts,
        root: d,
        includeDirs: false,
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

export interface RunTestModulesOptions extends Deno.RunTestsOptions {
  include?: string[];
  exclude?: string[];
  allowNone?: boolean;
}

/**
 * Renders test file that will be run.
 *
 * It's done to optimize compilation of test files, because
 * dynamically importing them one by one takes very long time.
 * @TODO(bartlomieju): try to optimize compilation by reusing same compiler host
 * multiple times
 * @param testModules
 */
function renderTestFile(testModules: string[]): string {
  let testFile = "";

  for (const testModule of testModules) {
    // NOTE: this is intentional that template string is not used
    // because of TS compiler quirkness of trying to import it
    // rather than treating it like a variable
    testFile += 'import "' + testModule + '"\n';
  }

  return testFile;
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
  exitOnFail = false,
  only = /[^\s]/,
  skip = /^\s*$/,
  disableLog = false,
}: RunTestModulesOptions = {}): Promise<void> {
  let moduleCount = 0;
  const testModules = [];
  for await (const testModule of findTestModules(include, exclude)) {
    testModules.push(testModule);
    moduleCount++;
  }

  if (moduleCount == 0) {
    const noneFoundMessage = "No matching test modules found.";
    if (!allowNone) {
      throw new Deno.errors.NotFound(noneFoundMessage);
    } else if (!disableLog) {
      console.log(noneFoundMessage);
    }
    return;
  }

  // Create temporary test file which contains
  // all matched modules as import statements.
  const testFile = renderTestFile(testModules);
  // Select where temporary test file will be stored.
  // If `DENO_DIR` is set it means that user intentionally wants to store
  // modules there - so it's a sane default to store there.
  // Fallback is current directory which again seems like a sane default,
  // user is probably working on project in this directory or even
  // cd'ed into current directory to quickly run test from this directory.
  const root = Deno.env("DENO_DIR") || Deno.cwd();
  const testFilePath = join(root, ".deno.test.ts");
  const data = new TextEncoder().encode(testFile);
  await Deno.writeFile(testFilePath, data);

  // Import temporary test file and delete it immediately after importing so it's not cluttering disk.
  //
  // You may think that this will cause recompilation on each run, but this actually
  // tricks Deno to not recompile files if there's no need.
  // Eg.
  //   1. On first run of $DENO_DIR/.deno.test.ts Deno will compile and cache temporary test file and all of its imports
  //   2. Temporary test file is removed by test runner
  //   3. On next test run file is created again. If no new modules were added then temporary file contents are identical.
  //      Deno will not compile temporary test file again, but load it directly into V8.
  //   4. Deno starts loading imports one by one.
  //   5. If imported file is outdated, Deno will recompile this single file.
  let err;
  try {
    await import(`file://${testFilePath}`);
  } catch (e) {
    err = e;
  } finally {
    await Deno.remove(testFilePath);
  }

  if (err) {
    throw err;
  }

  if (!disableLog) {
    console.log(`Found ${moduleCount} matching test modules.`);
  }

  await Deno.runTests({
    exitOnFail,
    only,
    skip,
    disableLog,
  });
}

async function main(): Promise<void> {
  const parsedArgs = parse(args, {
    boolean: ["allow-none", "failfast", "help", "quiet"],
    string: ["exclude"],
    alias: {
      exclude: ["e"],
      failfast: ["f"],
      help: ["h"],
      quiet: ["q"],
    },
    default: {
      "allow-none": false,
      failfast: false,
      help: false,
      quiet: false,
    },
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
  const disableLog = parsedArgs.quiet;

  try {
    await runTestModules({
      include,
      exclude,
      allowNone,
      disableLog,
      exitOnFail: true,
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
