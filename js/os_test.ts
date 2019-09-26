// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import {
  test,
  testPerm,
  assert,
  assertEquals,
  assertNotEquals
} from "./test_util.ts";

testPerm({ env: true }, function envSuccess(): void {
  const env = Deno.env();
  assert(env !== null);
  // eslint-disable-next-line @typescript-eslint/camelcase
  env.test_var = "Hello World";
  const newEnv = Deno.env();
  assertEquals(env.test_var, newEnv.test_var);
  assertEquals(Deno.env("test_var"), env.test_var);
});

test(function envFailure(): void {
  let caughtError = false;
  try {
    Deno.env();
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }

  assert(caughtError);
});

test(function envFailure(): void {
  let caughtError = false;
  try {
    Deno.env();
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }

  assert(caughtError);
});

if (Deno.build.os === "win") {
  testPerm({ env: true, run: true }, async function envCaseInsensitive() {
    // This test verifies that on Windows, environment variables are
    // case-insensitive. Case normalization needs be done using the collation
    // that Windows uses, rather than naively using String.toLowerCase().

    assertEquals(Deno.env("path"), Deno.env("PATH"));
    assertEquals(Deno.env("Path"), Deno.env("PATH"));

    // Utility function that runs a Deno subprocess with the environment
    // specified in `env`. The subprocess reads the environment variables
    // specified in `keys` and outputs them as a JSON object.
    const runGetEnv = async (env, keys) => {
      const src = `
        Deno.stdout.write((new TextEncoder()).encode(JSON.stringify(
          ${JSON.stringify(keys)}.map(k => Deno.env(k))
        )))`;
      const proc = Deno.run({
        args: [Deno.execPath(), "eval", src],
        env,
        stdout: "piped"
      });
      const [output, status] = await Promise.all([
        proc.output(),
        proc.status()
      ]);
      assertEquals(status.success, true);
      return JSON.parse(new TextDecoder().decode(output));
    };

    // That 'foo', 'Foo' and 'Foo' are case folded.
    assertEquals(await runGetEnv({ foo: "same" }, ["foo", "Foo", "FOO"]), [
      "same",
      "same",
      "same"
    ]);

    // Check that 'µ' and 'Μ' are not case folded.
    {
      const lc = "µ";
      const uc = lc.toUpperCase();
      assertNotEquals(lc, uc);
      assertEquals(await runGetEnv({ [lc]: "mu", [uc]: "MU" }, [lc, uc]), [
        "mu",
        "MU"
      ]);
    }
    {
      // Check that 'ǆ' and 'Ǆ' are folded, but 'ǅ' is ignored.
      const c = "ǅ";
      const lc = c.toLowerCase();
      const uc = c.toUpperCase();
      assertNotEquals(c, lc);
      assertNotEquals(c, uc);
      assertEquals(
        await runGetEnv({ [c]: "Dz", [lc]: "folded" }, [c, lc, uc]),
        ["Dz", "folded", "folded"]
      );
      assertEquals(await runGetEnv({ [lc]: "dz" }, [lc, uc]), ["dz", "dz"]);
      assertEquals(await runGetEnv({ [uc]: "DZ" }, [lc, uc]), ["DZ", "DZ"]);
    }
  });
}

test(function osPid(): void {
  console.log("pid", Deno.pid);
  assert(Deno.pid > 0);
});

// See complete tests in tools/is_tty_test.py
test(function osIsTTYSmoke(): void {
  console.log(Deno.isTTY());
});

testPerm({ env: true }, function homeDir(): void {
  assertNotEquals(Deno.homeDir(), "");
});

testPerm({ env: false }, function homeDirPerm(): void {
  let caughtError = false;
  try {
    Deno.homeDir();
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ env: true }, function execPath(): void {
  assertNotEquals(Deno.execPath(), "");
});

testPerm({ env: false }, function execPathPerm(): void {
  let caughtError = false;
  try {
    Deno.execPath();
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
  assert(caughtError);
});
