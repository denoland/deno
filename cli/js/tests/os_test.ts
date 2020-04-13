// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertThrows,
  unitTest,
} from "./test_util.ts";

unitTest({ perms: { env: true } }, function envSuccess(): void {
  const env = Deno.env();
  assert(env !== null);
  // eslint-disable-next-line @typescript-eslint/camelcase
  env.test_var = "Hello World";
  const newEnv = Deno.env();
  assertEquals(env.test_var, newEnv.test_var);
  assertEquals(Deno.env("test_var"), env.test_var);
});

unitTest({ perms: { env: true } }, function envNotFound(): void {
  const r = Deno.env("env_var_does_not_exist!");
  assertEquals(r, undefined);
});

unitTest(function envPermissionDenied1(): void {
  let err;
  try {
    Deno.env();
  } catch (e) {
    err = e;
  }
  assertNotEquals(err, undefined);
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

unitTest(function envPermissionDenied2(): void {
  let err;
  try {
    Deno.env("PATH");
  } catch (e) {
    err = e;
  }
  assertNotEquals(err, undefined);
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

// This test verifies that on Windows, environment variables are
// case-insensitive. Case normalization needs be done using the collation
// that Windows uses, rather than naively using String.toLowerCase().
unitTest(
  { ignore: Deno.build.os !== "win", perms: { env: true, run: true } },
  async function envCaseInsensitive() {
    // Utility function that runs a Deno subprocess with the environment
    // specified in `inputEnv`. The subprocess reads the environment variables
    // which are in the keys of `expectedEnv` and writes them to stdout as JSON.
    // It is then verified that these match with the values of `expectedEnv`.
    const checkChildEnv = async (
      inputEnv: Record<string, string>,
      expectedEnv: Record<string, string>
    ): Promise<void> => {
      const src = `
      console.log(
        ${JSON.stringify(Object.keys(expectedEnv))}.map(k => Deno.env(k))
      )`;
      const proc = Deno.run({
        cmd: [Deno.execPath(), "eval", src],
        env: inputEnv,
        stdout: "piped",
      });
      const status = await proc.status();
      assertEquals(status.success, true);
      const expectedValues = Object.values(expectedEnv);
      const actualValues = JSON.parse(
        new TextDecoder().decode(await proc.output())
      );
      assertEquals(actualValues, expectedValues);
      proc.close();
    };

    assertEquals(Deno.env("path"), Deno.env("PATH"));
    assertEquals(Deno.env("Path"), Deno.env("PATH"));

    // Check 'foo', 'Foo' and 'Foo' are case folded.
    await checkChildEnv({ foo: "X" }, { foo: "X", Foo: "X", FOO: "X" });

    // Check that 'µ' and 'Μ' are not case folded.
    const lc1 = "µ";
    const uc1 = lc1.toUpperCase();
    assertNotEquals(lc1, uc1);
    await checkChildEnv(
      { [lc1]: "mu", [uc1]: "MU" },
      { [lc1]: "mu", [uc1]: "MU" }
    );

    // Check that 'ǆ' and 'Ǆ' are folded, but 'ǅ' is preserved.
    const c2 = "ǅ";
    const lc2 = c2.toLowerCase();
    const uc2 = c2.toUpperCase();
    assertNotEquals(c2, lc2);
    assertNotEquals(c2, uc2);
    await checkChildEnv(
      { [c2]: "Dz", [lc2]: "dz" },
      { [c2]: "Dz", [lc2]: "dz", [uc2]: "dz" }
    );
    await checkChildEnv(
      { [c2]: "Dz", [uc2]: "DZ" },
      { [c2]: "Dz", [uc2]: "DZ", [lc2]: "DZ" }
    );
  }
);

unitTest(function osPid(): void {
  assert(Deno.pid > 0);
});

unitTest({ perms: { env: true } }, function getDir(): void {
  type supportOS = "mac" | "win" | "linux";

  interface Runtime {
    os: supportOS;
    shouldHaveValue: boolean;
  }

  interface Scenes {
    kind: Deno.DirKind;
    runtime: Runtime[];
  }

  const scenes: Scenes[] = [
    {
      kind: "config",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: true },
      ],
    },
    {
      kind: "cache",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: true },
      ],
    },
    {
      kind: "executable",
      runtime: [
        { os: "mac", shouldHaveValue: false },
        { os: "win", shouldHaveValue: false },
        { os: "linux", shouldHaveValue: true },
      ],
    },
    {
      kind: "data",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: true },
      ],
    },
    {
      kind: "data_local",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: true },
      ],
    },
    {
      kind: "audio",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false },
      ],
    },
    {
      kind: "desktop",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false },
      ],
    },
    {
      kind: "document",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false },
      ],
    },
    {
      kind: "download",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false },
      ],
    },
    {
      kind: "font",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: false },
        { os: "linux", shouldHaveValue: true },
      ],
    },
    {
      kind: "picture",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false },
      ],
    },
    {
      kind: "public",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false },
      ],
    },
    {
      kind: "template",
      runtime: [
        { os: "mac", shouldHaveValue: false },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false },
      ],
    },
    {
      kind: "tmp",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: true },
      ],
    },
    {
      kind: "video",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false },
      ],
    },
  ];

  for (const s of scenes) {
    for (const r of s.runtime) {
      if (Deno.build.os !== r.os) continue;
      if (r.shouldHaveValue) {
        const d = Deno.dir(s.kind);
        assert(d);
        assert(d.length > 0);
      }
    }
  }
});

unitTest(function getDirWithoutPermission(): void {
  assertThrows(
    () => Deno.dir("home"),
    Deno.errors.PermissionDenied,
    `run again with the --allow-env flag`
  );
});

unitTest({ perms: { env: true } }, function execPath(): void {
  assertNotEquals(Deno.execPath(), "");
});

unitTest({ perms: { env: false } }, function execPathPerm(): void {
  let caughtError = false;
  try {
    Deno.execPath();
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});

unitTest({ perms: { env: true } }, function loadavgSuccess(): void {
  const load = Deno.loadavg();
  assertEquals(load.length, 3);
});

unitTest({ perms: { env: false } }, function loadavgPerm(): void {
  let caughtError = false;
  try {
    Deno.loadavg();
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});

unitTest({ perms: { env: true } }, function hostnameDir(): void {
  assertNotEquals(Deno.hostname(), "");
});

unitTest({ perms: { env: false } }, function hostnamePerm(): void {
  let caughtError = false;
  try {
    Deno.hostname();
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});

unitTest({ perms: { env: true } }, function releaseDir(): void {
  assertNotEquals(Deno.osRelease(), "");
});

unitTest({ perms: { env: false } }, function releasePerm(): void {
  let caughtError = false;
  try {
    Deno.osRelease();
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});
