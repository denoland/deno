// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../../../std/testing/asserts.ts";
import * as colors from "../../../std/fmt/colors.ts";
export { colors };
import { resolve } from "../../../std/path/mod.ts";
export {
  assert,
  assertEquals,
  assertMatch,
  assertNotEquals,
  assertStrictEquals,
  assertStringIncludes,
  assertThrows,
  assertThrowsAsync,
  fail,
  unreachable,
} from "../../../std/testing/asserts.ts";
export { readLines } from "../../../std/io/bufio.ts";
export { parse as parseArgs } from "../../../std/flags/mod.ts";

export interface Permissions {
  read: boolean;
  write: boolean;
  net: boolean;
  env: boolean;
  run: boolean;
  plugin: boolean;
  hrtime: boolean;
}

export function fmtPerms(perms: Permissions): string {
  const p = Object.keys(perms)
    .filter((e): boolean => perms[e as keyof Permissions] === true)
    .map((key) => `--allow-${key}`);

  if (p.length) {
    return p.join(" ");
  }

  return "<no permissions>";
}

const isGranted = async (name: Deno.PermissionName): Promise<boolean> =>
  (await Deno.permissions.query({ name })).state === "granted";

export async function getProcessPermissions(): Promise<Permissions> {
  return {
    run: await isGranted("run"),
    read: await isGranted("read"),
    write: await isGranted("write"),
    net: await isGranted("net"),
    env: await isGranted("env"),
    plugin: await isGranted("plugin"),
    hrtime: await isGranted("hrtime"),
  };
}

export function permissionsMatch(
  processPerms: Permissions,
  requiredPerms: Permissions,
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

export async function registerUnitTests(): Promise<void> {
  const processPerms = await getProcessPermissions();

  const onlyTests = REGISTERED_UNIT_TESTS.filter(({ only }) => only);
  const unitTests = onlyTests.length > 0 ? onlyTests : REGISTERED_UNIT_TESTS;
  for (const unitTestDefinition of unitTests) {
    if (!permissionsMatch(processPerms, unitTestDefinition.perms)) {
      continue;
    }

    Deno.test(unitTestDefinition);
  }
}

function normalizeTestPermissions(perms: UnitTestPermissions): Permissions {
  return {
    read: !!perms.read,
    write: !!perms.write,
    net: !!perms.net,
    run: !!perms.run,
    env: !!perms.env,
    plugin: !!perms.plugin,
    hrtime: !!perms.hrtime,
  };
}

interface UnitTestPermissions {
  read?: boolean;
  write?: boolean;
  net?: boolean;
  env?: boolean;
  run?: boolean;
  plugin?: boolean;
  hrtime?: boolean;
}

interface UnitTestOptions {
  ignore?: boolean;
  only?: boolean;
  perms?: UnitTestPermissions;
}

interface UnitTestDefinition extends Deno.TestDefinition {
  ignore: boolean;
  only: boolean;
  perms: Permissions;
}

type TestFunction = () => void | Promise<void>;

export const REGISTERED_UNIT_TESTS: UnitTestDefinition[] = [];

export function unitTest(fn: TestFunction): void;
export function unitTest(options: UnitTestOptions, fn: TestFunction): void;
export function unitTest(
  optionsOrFn: UnitTestOptions | TestFunction,
  maybeFn?: TestFunction,
): void {
  assert(optionsOrFn, "At least one argument is required");

  let options: UnitTestOptions;
  let name: string;
  let fn: TestFunction;

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
      "Second argument should be test function definition",
    );
    fn = maybeFn;
    name = fn.name;
    assert(name, "Missing test function name");
  }

  const normalizedPerms = normalizeTestPermissions(options.perms || {});
  registerPermCombination(normalizedPerms);

  const unitTestDefinition: UnitTestDefinition = {
    name,
    fn,
    ignore: !!options.ignore,
    only: !!options.only,
    perms: normalizedPerms,
  };

  REGISTERED_UNIT_TESTS.push(unitTestDefinition);
}

export interface ResolvableMethods<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // deno-lint-ignore no-explicit-any
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

const encoder = new TextEncoder();

// Replace functions with null, errors with their stack strings, and JSONify.
// deno-lint-ignore no-explicit-any
function serializeTestMessage(message: any): string {
  return JSON.stringify({
    start: message.start && {
      ...message.start,
      tests: message.start.tests.map((test: Deno.TestDefinition) => ({
        ...test,
        fn: null,
      })),
    },
    testStart: message.testStart && { ...message.testStart, fn: null },
    testEnd: message.testEnd && {
      ...message.testEnd,
      error: String(message.testEnd.error?.stack),
    },
    end: message.end && {
      ...message.end,
      // deno-lint-ignore no-explicit-any
      results: message.end.results.map((result: any) => ({
        ...result,
        error: result.error?.stack,
      })),
    },
  });
}

export async function reportToConn(
  conn: Deno.Conn,
  // deno-lint-ignore no-explicit-any
  message: any,
): Promise<void> {
  const line = serializeTestMessage(message);
  const encodedMsg = encoder.encode(line + (message.end == null ? "\n" : ""));
  await Deno.writeAll(conn, encodedMsg);
  if (message.end != null) {
    conn.closeWrite();
  }
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
        hrtime: false,
      },
      normalizeTestPermissions({ read: true }),
    ),
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
        hrtime: false,
      },
      normalizeTestPermissions({}),
    ),
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
        hrtime: true,
      },
      normalizeTestPermissions({ read: true }),
    ),
    false,
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
        hrtime: false,
      },
      normalizeTestPermissions({ read: true }),
    ),
    false,
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
        hrtime: true,
      },
      {
        read: true,
        write: true,
        net: true,
        env: true,
        run: true,
        plugin: true,
        hrtime: true,
      },
    ),
  );
});

/*
 * Ensure all unit test files (e.g. xxx_test.ts) are present as imports in
 * cli/tests/unit/unit_tests.ts as it is easy to miss this out
 */
unitTest(
  { perms: { read: true } },
  function assertAllUnitTestFilesImported(): void {
    const directoryTestFiles = [...Deno.readDirSync("./cli/tests/unit/")]
      .map((k) => k.name)
      .filter(
        (file) =>
          file!.endsWith(".ts") &&
          !file!.endsWith("unit_tests.ts") &&
          !file!.endsWith("test_util.ts") &&
          !file!.endsWith("unit_test_runner.ts"),
      );
    const unitTestsFile: Uint8Array = Deno.readFileSync(
      "./cli/tests/unit/unit_tests.ts",
    );
    const importLines = new TextDecoder("utf-8")
      .decode(unitTestsFile)
      .split("\n")
      .filter((line) => line.startsWith("import"));
    const importedTestFiles = importLines.map(
      (relativeFilePath) => relativeFilePath.match(/\/([^\/]+)";/)![1],
    );

    directoryTestFiles.forEach((dirFile) => {
      if (!importedTestFiles.includes(dirFile!)) {
        throw new Error(
          "cil/tests/unit/unit_tests.ts is missing import of test file: cli/js/" +
            dirFile,
        );
      }
    });
  },
);

export function pathToAbsoluteFileUrl(path: string): URL {
  path = resolve(path);

  return new URL(`file://${Deno.build.os === "windows" ? "/" : ""}${path}`);
}
