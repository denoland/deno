// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert } from "./test_util.ts";

testPerm({ env: true }, function envSuccess(): void {
  const env = Deno.env();
  assert(env !== null);
  // eslint-disable-next-line @typescript-eslint/camelcase
  env.test_var = "Hello World";
  const newEnv = Deno.env();
  assert.equals(env.test_var, newEnv.test_var);
  assert.equals(Deno.env("test_var"), env.test_var);
});

testPerm({ env: true }, function envNotFound(): void {
  const r = Deno.env("env_var_does_not_exist!");
  assert.equals(r, undefined);
});

test(function envPermissionDenied1(): void {
  let err;
  try {
    Deno.env();
  } catch (e) {
    err = e;
  }
  assert.notEquals(err, undefined);
  assert(err instanceof Deno.errors.PermissionDenied);
  assert.equals(err.name, "PermissionDenied");
});

test(function envPermissionDenied2(): void {
  let err;
  try {
    Deno.env("PATH");
  } catch (e) {
    err = e;
  }
  assert.notEquals(err, undefined);
  assert(err instanceof Deno.errors.PermissionDenied);
  assert.equals(err.name, "PermissionDenied");
});

if (Deno.build.os === "win") {
  // This test verifies that on Windows, environment variables are
  // case-insensitive. Case normalization needs be done using the collation
  // that Windows uses, rather than naively using String.toLowerCase().
  testPerm({ env: true, run: true }, async function envCaseInsensitive() {
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
        args: [Deno.execPath(), "eval", src],
        env: inputEnv,
        stdout: "piped"
      });
      const status = await proc.status();
      assert.equals(status.success, true);
      const expectedValues = Object.values(expectedEnv);
      const actualValues = JSON.parse(
        new TextDecoder().decode(await proc.output())
      );
      assert.equals(actualValues, expectedValues);
    };

    assert.equals(Deno.env("path"), Deno.env("PATH"));
    assert.equals(Deno.env("Path"), Deno.env("PATH"));

    // Check 'foo', 'Foo' and 'Foo' are case folded.
    await checkChildEnv({ foo: "X" }, { foo: "X", Foo: "X", FOO: "X" });

    // Check that 'µ' and 'Μ' are not case folded.
    const lc1 = "µ";
    const uc1 = lc1.toUpperCase();
    assert.notEquals(lc1, uc1);
    await checkChildEnv(
      { [lc1]: "mu", [uc1]: "MU" },
      { [lc1]: "mu", [uc1]: "MU" }
    );

    // Check that 'ǆ' and 'Ǆ' are folded, but 'ǅ' is preserved.
    const c2 = "ǅ";
    const lc2 = c2.toLowerCase();
    const uc2 = c2.toUpperCase();
    assert.notEquals(c2, lc2);
    assert.notEquals(c2, uc2);
    await checkChildEnv(
      { [c2]: "Dz", [lc2]: "dz" },
      { [c2]: "Dz", [lc2]: "dz", [uc2]: "dz" }
    );
    await checkChildEnv(
      { [c2]: "Dz", [uc2]: "DZ" },
      { [c2]: "Dz", [uc2]: "DZ", [lc2]: "DZ" }
    );
  });
}

test(function osPid(): void {
  console.log("pid", Deno.pid);
  assert(Deno.pid > 0);
});

testPerm({ env: true }, function getDir(): void {
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
        { os: "linux", shouldHaveValue: true }
      ]
    },
    {
      kind: "cache",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: true }
      ]
    },
    {
      kind: "executable",
      runtime: [
        { os: "mac", shouldHaveValue: false },
        { os: "win", shouldHaveValue: false },
        { os: "linux", shouldHaveValue: true }
      ]
    },
    {
      kind: "data",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: true }
      ]
    },
    {
      kind: "data_local",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: true }
      ]
    },
    {
      kind: "audio",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false }
      ]
    },
    {
      kind: "desktop",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false }
      ]
    },
    {
      kind: "document",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false }
      ]
    },
    {
      kind: "download",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false }
      ]
    },
    {
      kind: "font",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: false },
        { os: "linux", shouldHaveValue: true }
      ]
    },
    {
      kind: "picture",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false }
      ]
    },
    {
      kind: "public",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false }
      ]
    },
    {
      kind: "template",
      runtime: [
        { os: "mac", shouldHaveValue: false },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false }
      ]
    },
    {
      kind: "video",
      runtime: [
        { os: "mac", shouldHaveValue: true },
        { os: "win", shouldHaveValue: true },
        { os: "linux", shouldHaveValue: false }
      ]
    }
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

testPerm({}, function getDirWithoutPermission(): void {
  assert.throws(
    () => Deno.dir("home"),
    Deno.errors.PermissionDenied,
    `run again with the --allow-env flag`
  );
});

testPerm({ env: true }, function execPath(): void {
  assert.notEquals(Deno.execPath(), "");
});

testPerm({ env: false }, function execPathPerm(): void {
  let caughtError = false;
  try {
    Deno.execPath();
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.PermissionDenied);
    assert.equals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ env: true }, function loadavgSuccess(): void {
  const load = Deno.loadavg();
  assert.equals(load.length, 3);
});

testPerm({ env: false }, function loadavgPerm(): void {
  let caughtError = false;
  try {
    Deno.loadavg();
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.PermissionDenied);
    assert.equals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ env: true }, function hostnameDir(): void {
  assert.notEquals(Deno.hostname(), "");
});

testPerm({ env: false }, function hostnamePerm(): void {
  let caughtError = false;
  try {
    Deno.hostname();
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.PermissionDenied);
    assert.equals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ env: true }, function releaseDir(): void {
  assert.notEquals(Deno.osRelease(), "");
});

testPerm({ env: false }, function releasePerm(): void {
  let caughtError = false;
  try {
    Deno.osRelease();
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.PermissionDenied);
    assert.equals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});
