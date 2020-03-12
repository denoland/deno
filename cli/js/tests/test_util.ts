// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//
// We want to test many ops in deno which have different behavior depending on
// the permissions set. These tests can specify which permissions they expect,
// which appends a special string like "permW1N0" to the end of the test name.
// Here we run several copies of deno with different permissions, filtering the
// tests by the special string. permW1N0 means allow-write but not allow-net.
// See tools/unit_tests.py for more details.

import { readLines } from "../../../std/io/bufio.ts";
import { assert, assertEquals } from "../../../std/testing/asserts.ts";
export {
  assert,
  assertThrows,
  assertEquals,
  assertMatch,
  assertNotEquals,
  assertStrictEq,
  assertStrContains,
  unreachable,
  fail
} from "../../../std/testing/asserts.ts";

interface TestPermissions {
  read?: boolean;
  write?: boolean;
  net?: boolean;
  env?: boolean;
  run?: boolean;
  plugin?: boolean;
  hrtime?: boolean;
}

export interface Permissions {
  read: boolean;
  write: boolean;
  net: boolean;
  env: boolean;
  run: boolean;
  plugin: boolean;
  hrtime: boolean;
}

const isGranted = async (name: Deno.PermissionName): Promise<boolean> =>
  (await Deno.permissions.query({ name })).state === "granted";

async function getProcessPermissions(): Promise<Permissions> {
  return {
    run: await isGranted("run"),
    read: await isGranted("read"),
    write: await isGranted("write"),
    net: await isGranted("net"),
    env: await isGranted("env"),
    plugin: await isGranted("plugin"),
    hrtime: await isGranted("hrtime")
  };
}

const processPerms = await getProcessPermissions();

function permissionsMatch(
  processPerms: Permissions,
  requiredPerms: Permissions
): boolean {
  for (const permName in processPerms) {
    if (
      processPerms[permName as keyof Permissions] !==
      requiredPerms[permName as keyof Permissions]
    ) {
      return false;
    }
  }

  return true;
}

export const permissionCombinations: Map<string, Permissions> = new Map();

function permToString(perms: Permissions): string {
  const r = perms.read ? 1 : 0;
  const w = perms.write ? 1 : 0;
  const n = perms.net ? 1 : 0;
  const e = perms.env ? 1 : 0;
  const u = perms.run ? 1 : 0;
  const p = perms.plugin ? 1 : 0;
  const h = perms.hrtime ? 1 : 0;
  return `permR${r}W${w}N${n}E${e}U${u}P${p}H${h}`;
}

function registerPermCombination(perms: Permissions): void {
  const key = permToString(perms);
  if (!permissionCombinations.has(key)) {
    permissionCombinations.set(key, perms);
  }
}

function normalizeTestPermissions(perms: TestPermissions): Permissions {
  return {
    read: !!perms.read,
    write: !!perms.write,
    net: !!perms.net,
    run: !!perms.run,
    env: !!perms.env,
    plugin: !!perms.plugin,
    hrtime: !!perms.hrtime
  };
}

// Wrap `TestFunction` in additional assertion that makes sure
// the test case does not leak async "ops" - ie. number of async
// completed ops after the test is the same as number of dispatched
// ops. Note that "unref" ops are ignored since in nature that are
// optional.
function assertOps(fn: Deno.TestFunction): Deno.TestFunction {
  return async function asyncOpSanitizer(): Promise<void> {
    const pre = Deno.metrics();
    await fn();
    const post = Deno.metrics();
    // We're checking diff because one might spawn HTTP server in the background
    // that will be a pending async op before test starts.
    assertEquals(
      post.opsDispatchedAsync - pre.opsDispatchedAsync,
      post.opsCompletedAsync - pre.opsCompletedAsync,
      `Test case is leaking async ops.
    Before:
      - dispatched: ${pre.opsDispatchedAsync}
      - completed: ${pre.opsCompletedAsync}
    After: 
      - dispatched: ${post.opsDispatchedAsync}
      - completed: ${post.opsCompletedAsync}`
    );
  };
}

// Wrap `TestFunction` in additional assertion that makes sure
// the test case does not "leak" resources - ie. resource table after
// the test has exactly the same contents as before the test.
function assertResources(fn: Deno.TestFunction): Deno.TestFunction {
  return async function resourceSanitizer(): Promise<void> {
    const pre = Deno.resources();
    await fn();
    const post = Deno.resources();
    const msg = `Test case is leaking resources.
    Before: ${JSON.stringify(pre, null, 2)}
    After: ${JSON.stringify(post, null, 2)}`;
    assertEquals(pre, post, msg);
  };
}

interface UnitTestOptions {
  skip?: boolean;
  perms?: TestPermissions;
}

export function unitTest(fn: Deno.TestFunction): void;
export function unitTest(options: UnitTestOptions, fn: Deno.TestFunction): void;
export function unitTest(
  optionsOrFn: UnitTestOptions | Deno.TestFunction,
  maybeFn?: Deno.TestFunction
): void {
  assert(optionsOrFn, "At least one argument is required");

  let options: UnitTestOptions;
  let name: string;
  let fn: Deno.TestFunction;

  if (typeof optionsOrFn === "function") {
    options = {};
    fn = optionsOrFn;
    name = fn.name;
    assert(name, "Missing test function name");
  } else {
    options = optionsOrFn;
    assert(maybeFn, "Missing test function definition");
    assert(
      typeof maybeFn === "function",
      "Second argument should be test function definition"
    );
    fn = maybeFn;
    name = fn.name;
    assert(name, "Missing test function name");
  }

  if (options.skip) {
    return;
  }

  const normalizedPerms = normalizeTestPermissions(options.perms || {});
  registerPermCombination(normalizedPerms);
  if (!permissionsMatch(processPerms, normalizedPerms)) {
    return;
  }

  const testDefinition: Deno.TestDefinition = {
    name,
    fn: assertResources(assertOps(fn))
  };
  Deno.test(testDefinition);
}

function extractNumber(re: RegExp, str: string): number | undefined {
  const match = str.match(re);

  if (match) {
    return Number.parseInt(match[1]);
  }
}

export async function parseUnitTestOutput(
  reader: Deno.Reader,
  print: boolean
): Promise<{ actual?: number; expected?: number; resultOutput?: string }> {
  let expected, actual, result;

  for await (const line of readLines(reader)) {
    if (!expected) {
      // expect "running 30 tests"
      expected = extractNumber(/running (\d+) tests/, line);
    } else if (line.indexOf("test result:") !== -1) {
      result = line;
    }

    if (print) {
      console.log(line);
    }
  }

  // Check that the number of expected tests equals what was reported at the
  // bottom.
  if (result) {
    // result should be a string like this:
    // "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ..."
    actual = extractNumber(/(\d+) passed/, result);
  }

  return { actual, expected, resultOutput: result };
}

export interface ResolvableMethods<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  reject: (reason?: any) => void;
}

export type Resolvable<T> = Promise<T> & ResolvableMethods<T>;

export function createResolvable<T>(): Resolvable<T> {
  let methods: ResolvableMethods<T>;
  const promise = new Promise<T>((resolve, reject): void => {
    methods = { resolve, reject };
  });
  // TypeScript doesn't know that the Promise callback occurs synchronously
  // therefore use of not null assertion (`!`)
  return Object.assign(promise, methods!) as Resolvable<T>;
}

unitTest(function permissionsMatches(): void {
  assert(
    permissionsMatch(
      {
        read: true,
        write: false,
        net: false,
        env: false,
        run: false,
        plugin: false,
        hrtime: false
      },
      normalizeTestPermissions({ read: true })
    )
  );

  assert(
    permissionsMatch(
      {
        read: false,
        write: false,
        net: false,
        env: false,
        run: false,
        plugin: false,
        hrtime: false
      },
      normalizeTestPermissions({})
    )
  );

  assertEquals(
    permissionsMatch(
      {
        read: false,
        write: true,
        net: true,
        env: true,
        run: true,
        plugin: true,
        hrtime: true
      },
      normalizeTestPermissions({ read: true })
    ),
    false
  );

  assertEquals(
    permissionsMatch(
      {
        read: true,
        write: false,
        net: true,
        env: false,
        run: false,
        plugin: false,
        hrtime: false
      },
      normalizeTestPermissions({ read: true })
    ),
    false
  );

  assert(
    permissionsMatch(
      {
        read: true,
        write: true,
        net: true,
        env: true,
        run: true,
        plugin: true,
        hrtime: true
      },
      {
        read: true,
        write: true,
        net: true,
        env: true,
        run: true,
        plugin: true,
        hrtime: true
      }
    )
  );
});

unitTest(
  { perms: { read: true } },
  async function parsingUnitTestOutput(): Promise<void> {
    const cwd = Deno.cwd();
    const testDataPath = `${cwd}/tools/testdata/`;

    let result;

    // This is an example of a successful unit test output.
    const f1 = await Deno.open(`${testDataPath}/unit_test_output1.txt`);
    result = await parseUnitTestOutput(f1, false);
    assertEquals(result.actual, 96);
    assertEquals(result.expected, 96);
    f1.close();

    // This is an example of a silently dying unit test.
    const f2 = await Deno.open(`${testDataPath}/unit_test_output2.txt`);
    result = await parseUnitTestOutput(f2, false);
    assertEquals(result.actual, undefined);
    assertEquals(result.expected, 96);
    f2.close();

    // This is an example of compiling before successful unit tests.
    const f3 = await Deno.open(`${testDataPath}/unit_test_output3.txt`);
    result = await parseUnitTestOutput(f3, false);
    assertEquals(result.actual, 96);
    assertEquals(result.expected, 96);
    f3.close();

    // Check what happens on empty output.
    const f = new Deno.Buffer(new TextEncoder().encode("\n\n\n"));
    result = await parseUnitTestOutput(f, false);
    assertEquals(result.actual, undefined);
    assertEquals(result.expected, undefined);
  }
);

/*
 * Ensure all unit test files (e.g. xxx_test.ts) are present as imports in
 * cli/js/tests/unit_tests.ts as it is easy to miss this out
 */
unitTest(
  { perms: { read: true } },
  async function assertAllUnitTestFilesImported(): Promise<void> {
    const directoryTestFiles = Deno.readdirSync("./cli/js/tests/")
      .map(k => k.name)
      .filter(
        file =>
          file!.endsWith(".ts") &&
          !file!.endsWith("unit_tests.ts") &&
          !file!.endsWith("test_util.ts") &&
          !file!.endsWith("unit_test_runner.ts")
      );
    const unitTestsFile: Uint8Array = Deno.readFileSync(
      "./cli/js/tests/unit_tests.ts"
    );
    const importLines = new TextDecoder("utf-8")
      .decode(unitTestsFile)
      .split("\n")
      .filter(line => line.startsWith("import"));
    const importedTestFiles = importLines.map(
      relativeFilePath => relativeFilePath.match(/\/([^\/]+)";/)![1]
    );

    directoryTestFiles.forEach(dirFile => {
      if (!importedTestFiles.includes(dirFile!)) {
        throw new Error(
          "cil/js/tests/unit_tests.ts is missing import of test file: cli/js/" +
            dirFile
        );
      }
    });
  }
);
