// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertThrows,
} from "./test_util.ts";

Deno.test("envSuccess", function (): void {
  Deno.env.set("TEST_VAR", "A");
  const env = Deno.env.toObject();
  Deno.env.set("TEST_VAR", "B");
  assertEquals(env["TEST_VAR"], "A");
  assertNotEquals(Deno.env.get("TEST_VAR"), env["TEST_VAR"]);
});

Deno.test("envNotFound", function (): void {
  const r = Deno.env.get("env_var_does_not_exist!");
  assertEquals(r, undefined);
});

Deno.test("deleteEnv", function (): void {
  Deno.env.set("TEST_VAR", "A");
  assertEquals(Deno.env.get("TEST_VAR"), "A");
  assertEquals(Deno.env.delete("TEST_VAR"), undefined);
  assertEquals(Deno.env.get("TEST_VAR"), undefined);
});

Deno.test("avoidEmptyNamedEnv", function (): void {
  assertThrows(() => Deno.env.set("", "v"), TypeError);
  assertThrows(() => Deno.env.set("a=a", "v"), TypeError);
  assertThrows(() => Deno.env.set("a\0a", "v"), TypeError);
  assertThrows(() => Deno.env.set("TEST_VAR", "v\0v"), TypeError);

  assertThrows(() => Deno.env.get(""), TypeError);
  assertThrows(() => Deno.env.get("a=a"), TypeError);
  assertThrows(() => Deno.env.get("a\0a"), TypeError);

  assertThrows(() => Deno.env.delete(""), TypeError);
  assertThrows(() => Deno.env.delete("a=a"), TypeError);
  assertThrows(() => Deno.env.delete("a\0a"), TypeError);
});

// This test verifies that on Windows, environment variables are
// case-insensitive. Case normalization needs be done using the collation
// that Windows uses, rather than naively using String.toLowerCase().
Deno.test({
  name: "envCaseInsensitive",
  ignore: Deno.build.os !== "windows",
  async fn() {
    // Utility function that runs a Deno subprocess with the environment
    // specified in `inputEnv`. The subprocess reads the environment variables
    // which are in the keys of `expectedEnv` and writes them to stdout as JSON.
    // It is then verified that these match with the values of `expectedEnv`.
    const checkChildEnv = async (
      inputEnv: Record<string, string>,
      expectedEnv: Record<string, string>,
    ): Promise<void> => {
      const src = `
      console.log(
        ${JSON.stringify(Object.keys(expectedEnv))}.map(k => Deno.env.get(k))
      )`;
      const proc = Deno.run({
        cmd: [Deno.execPath(), "eval", src],
        env: { ...inputEnv, NO_COLOR: "1" },
        stdout: "piped",
      });
      const status = await proc.status();
      assertEquals(status.success, true);
      const expectedValues = Object.values(expectedEnv);
      const actualValues = JSON.parse(
        new TextDecoder().decode(await proc.output()),
      );
      assertEquals(actualValues, expectedValues);
      proc.close();
    };

    assertEquals(Deno.env.get("path"), Deno.env.get("PATH"));
    assertEquals(Deno.env.get("Path"), Deno.env.get("PATH"));

    // Check 'foo', 'Foo' and 'Foo' are case folded.
    await checkChildEnv({ foo: "X" }, { foo: "X", Foo: "X", FOO: "X" });

    // Check that 'µ' and 'Μ' are not case folded.
    const lc1 = "µ";
    const uc1 = lc1.toUpperCase();
    assertNotEquals(lc1, uc1);
    await checkChildEnv(
      { [lc1]: "mu", [uc1]: "MU" },
      { [lc1]: "mu", [uc1]: "MU" },
    );

    // Check that 'ǆ' and 'Ǆ' are folded, but 'ǅ' is preserved.
    const c2 = "ǅ";
    const lc2 = c2.toLowerCase();
    const uc2 = c2.toUpperCase();
    assertNotEquals(c2, lc2);
    assertNotEquals(c2, uc2);
    await checkChildEnv(
      { [c2]: "Dz", [lc2]: "dz" },
      { [c2]: "Dz", [lc2]: "dz", [uc2]: "dz" },
    );
    await checkChildEnv(
      { [c2]: "Dz", [uc2]: "DZ" },
      { [c2]: "Dz", [uc2]: "DZ", [lc2]: "DZ" },
    );
  },
});

Deno.test("osPid", function (): void {
  assert(Deno.pid > 0);
});

Deno.test("osPpid", function (): void {
  assert(Deno.ppid > 0);
});

Deno.test("osPpidIsEqualToPidOfParentProcess", async function (): Promise<
  void
> {
  const decoder = new TextDecoder();
  const process = Deno.run({
    cmd: [Deno.execPath(), "eval", "-p", "--unstable", "Deno.ppid"],
    stdout: "piped",
    env: { NO_COLOR: "true" },
  });
  const output = await process.output();
  process.close();

  const expected = Deno.pid;
  const actual = parseInt(decoder.decode(output));
  assertEquals(actual, expected);
});

Deno.test("execPath", function (): void {
  assertNotEquals(Deno.execPath(), "");
});

Deno.test("loadavgSuccess", function (): void {
  const load = Deno.loadavg();
  assertEquals(load.length, 3);
});

Deno.test("hostnameDir", function (): void {
  assertNotEquals(Deno.hostname(), "");
});

Deno.test("releaseDir", function (): void {
  assertNotEquals(Deno.osRelease(), "");
});

Deno.test("systemMemoryInfo", function (): void {
  const info = Deno.systemMemoryInfo();
  assert(info.total >= 0);
  assert(info.free >= 0);
  assert(info.available >= 0);
  assert(info.buffers >= 0);
  assert(info.cached >= 0);
  assert(info.swapTotal >= 0);
  assert(info.swapFree >= 0);
});

Deno.test("systemCpuInfo", function (): void {
  const { cores, speed } = Deno.systemCpuInfo();
  assert(cores === undefined || cores > 0);
  assert(speed === undefined || speed > 0);
});

Deno.test("envPermissionDenied1", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "env" });

  assertThrows(() => {
    Deno.env.toObject();
  }, Deno.errors.PermissionDenied);
});

Deno.test("envPermissionDenied2", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "env" });

  assertThrows(() => {
    Deno.env.get("PATH");
  }, Deno.errors.PermissionDenied);
});

Deno.test("releasePerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "env" });

  assertThrows(() => {
    Deno.osRelease();
  }, Deno.errors.PermissionDenied);
});

Deno.test("loadavgPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "env" });

  assertThrows(() => {
    Deno.loadavg();
  }, Deno.errors.PermissionDenied);
});

Deno.test("hostnamePerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "env" });

  assertThrows(() => {
    Deno.hostname();
  }, Deno.errors.PermissionDenied);
});

Deno.test("execPathPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(
    () => {
      Deno.execPath();
    },
    Deno.errors.PermissionDenied,
    "Requires read access to <exec_path>, run again with the --allow-read flag",
  );
});
