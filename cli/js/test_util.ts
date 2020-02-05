// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//
// We want to test many ops in deno which have different behavior depending on
// the permissions set. These tests can specify which permissions they expect,
// which appends a special string like "permW1N0" to the end of the test name.
// Here we run several copies of deno with different permissions, filtering the
// tests by the special string. permW1N0 means allow-write but not allow-net.
// See tools/unit_tests.py for more details.

import { assert, assertEquals } from "../../std/testing/asserts.ts";
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
} from "../../std/testing/asserts.ts";

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
    if (processPerms[permName] !== requiredPerms[permName]) {
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

export function testPerm(perms: TestPermissions, fn: Deno.TestFunction): void {
  const normalizedPerms = normalizeTestPermissions(perms);

  registerPermCombination(normalizedPerms);

  if (!permissionsMatch(processPerms, normalizedPerms)) {
    return;
  }

  Deno.test(fn);
}

export function test(fn: Deno.TestFunction): void {
  testPerm(
    {
      read: false,
      write: false,
      net: false,
      env: false,
      run: false,
      plugin: false,
      hrtime: false
    },
    fn
  );
}

function extractNumber(re: RegExp, str: string): number | undefined {
  const match = str.match(re);

  if (match) {
    return Number.parseInt(match[1]);
  }
}

export function parseUnitTestOutput(
  rawOutput: Uint8Array,
  print: boolean
): { actual?: number; expected?: number; resultOutput?: string } {
  const decoder = new TextDecoder();
  const output = decoder.decode(rawOutput);

  let expected, actual, result;

  for (const line of output.split("\n")) {
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

test(function permissionsMatches(): void {
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

testPerm({ read: true }, async function parsingUnitTestOutput(): Promise<void> {
  const cwd = Deno.cwd();
  const testDataPath = `${cwd}/tools/testdata/`;

  let result;

  // This is an example of a successful unit test output.
  result = parseUnitTestOutput(
    await Deno.readFile(`${testDataPath}/unit_test_output1.txt`),
    false
  );
  assertEquals(result.actual, 96);
  assertEquals(result.expected, 96);

  // This is an example of a silently dying unit test.
  result = parseUnitTestOutput(
    await Deno.readFile(`${testDataPath}/unit_test_output2.txt`),
    false
  );
  assertEquals(result.actual, undefined);
  assertEquals(result.expected, 96);

  // This is an example of compiling before successful unit tests.
  result = parseUnitTestOutput(
    await Deno.readFile(`${testDataPath}/unit_test_output3.txt`),
    false
  );
  assertEquals(result.actual, 96);
  assertEquals(result.expected, 96);

  // Check what happens on empty output.
  result = parseUnitTestOutput(new TextEncoder().encode("\n\n\n"), false);
  assertEquals(result.actual, undefined);
  assertEquals(result.expected, undefined);
});

/*
 * Ensure all unit test files (e.g. xxx_test.ts) are present as imports in
 * cli/js/unit_tests.ts as it is easy to miss this out
 */
testPerm(
  { read: true },
  async function assertAllUnitTestFilesImported(): Promise<void> {
    const directoryTestFiles = Deno.readDirSync("./cli/js")
      .map(k => k.name)
      .filter(file => file.endsWith("_test.ts"));
    const unitTestsFile: Uint8Array = Deno.readFileSync(
      "./cli/js/unit_tests.ts"
    );
    const importLines = new TextDecoder("utf-8")
      .decode(unitTestsFile)
      .split("\n")
      .filter(line => line.startsWith("import") && line.includes("_test.ts"));
    const importedTestFiles = importLines.map(
      relativeFilePath => relativeFilePath.match(/\/([^\/]+)";/)[1]
    );

    directoryTestFiles.forEach(dirFile => {
      if (!importedTestFiles.includes(dirFile)) {
        throw new Error(
          "cil/js/unit_tests.ts is missing import of test file: cli/js/" +
            dirFile
        );
      }
    });
  }
);
