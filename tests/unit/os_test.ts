// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertThrows,
} from "./test_util.ts";

Deno.test({ permissions: { env: true } }, function envSuccess() {
  Deno.env.set("TEST_VAR", "A");
  const env = Deno.env.toObject();
  Deno.env.set("TEST_VAR", "B");
  assertEquals(env["TEST_VAR"], "A");
  assertNotEquals(Deno.env.get("TEST_VAR"), env["TEST_VAR"]);
});

Deno.test({ permissions: { env: true } }, function envNotFound() {
  const r = Deno.env.get("env_var_does_not_exist!");
  assertEquals(r, undefined);
});

Deno.test({ permissions: { env: true } }, function deleteEnv() {
  Deno.env.set("TEST_VAR", "A");
  assertEquals(Deno.env.get("TEST_VAR"), "A");
  assertEquals(Deno.env.delete("TEST_VAR"), undefined);
  assertEquals(Deno.env.get("TEST_VAR"), undefined);
});

Deno.test({ permissions: { env: true } }, function hasEnv() {
  Deno.env.set("TEST_VAR", "A");
  assert(Deno.env.has("TEST_VAR"));
  Deno.env.delete("TEST_VAR");
  assert(!Deno.env.has("TEST_VAR"));
});

Deno.test({ permissions: { env: true } }, function avoidEmptyNamedEnv() {
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

Deno.test({ permissions: { env: false } }, function envPerm1() {
  assertThrows(() => {
    Deno.env.toObject();
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { env: false } }, function envPerm2() {
  assertThrows(() => {
    Deno.env.get("PATH");
  }, Deno.errors.NotCapable);
});

// This test verifies that on Windows, environment variables are
// case-insensitive. Case normalization needs be done using the collation
// that Windows uses, rather than naively using String.toLowerCase().
Deno.test(
  {
    ignore: Deno.build.os !== "windows",
    permissions: { read: true, env: true, run: true },
  },
  async function envCaseInsensitive() {
    // Utility function that runs a Deno subprocess with the environment
    // specified in `inputEnv`. The subprocess reads the environment variables
    // which are in the keys of `expectedEnv` and writes them to stdout as JSON.
    // It is then verified that these match with the values of `expectedEnv`.
    const checkChildEnv = async (
      inputEnv: Record<string, string>,
      expectedEnv: Record<string, string>,
    ) => {
      const src = `
      console.log(
        ${
        JSON.stringify(Object.keys(expectedEnv))
      }.map(k => Deno.env.get(k) ?? null)
      )`;
      const { success, stdout } = await new Deno.Command(Deno.execPath(), {
        args: ["eval", src],
        env: { ...inputEnv, NO_COLOR: "1" },
      }).output();
      assertEquals(success, true);
      const expectedValues = Object.values(expectedEnv);
      const actualValues = JSON.parse(new TextDecoder().decode(stdout));
      assertEquals(actualValues, expectedValues);
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
);

Deno.test({ permissions: { env: true } }, function envInvalidChars() {
  assertThrows(() => Deno.env.get(""), TypeError, "Key is an empty string");
  assertThrows(
    () => Deno.env.get("\0"),
    TypeError,
    'Key contains invalid characters: "\\0"',
  );
  assertThrows(
    () => Deno.env.get("="),
    TypeError,
    'Key contains invalid characters: "="',
  );
  assertThrows(
    () => Deno.env.set("", "foo"),
    TypeError,
    "Key is an empty string",
  );
  assertThrows(
    () => Deno.env.set("\0", "foo"),
    TypeError,
    'Key contains invalid characters: "\\0"',
  );
  assertThrows(
    () => Deno.env.set("=", "foo"),
    TypeError,
    'Key contains invalid characters: "="',
  );
  assertThrows(
    () => Deno.env.set("foo", "\0"),
    TypeError,
    'Value contains invalid characters: "\\0"',
  );
});

Deno.test(function osPid() {
  assertEquals(typeof Deno.pid, "number");
  assert(Deno.pid > 0);
});

Deno.test(function osPpid() {
  assertEquals(typeof Deno.ppid, "number");
  assert(Deno.ppid > 0);
});

Deno.test(
  { permissions: { run: true, read: true } },
  async function osPpidIsEqualToPidOfParentProcess() {
    const decoder = new TextDecoder();
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "-p", "Deno.ppid"],
      env: { NO_COLOR: "true" },
    }).output();

    const expected = Deno.pid;
    const actual = parseInt(decoder.decode(stdout));
    assertEquals(actual, expected);
  },
);

Deno.test({ permissions: { read: true } }, function execPath() {
  assertNotEquals(Deno.execPath(), "");
});

Deno.test({ permissions: { read: false } }, function execPathPerm() {
  assertThrows(
    () => {
      Deno.execPath();
    },
    Deno.errors.NotCapable,
    "Requires read access to <exec_path>, run again with the --allow-read flag",
  );
});

Deno.test(
  {
    ignore: Deno.build.os !== "linux",
    permissions: { read: true, run: false },
  },
  function procRequiresAllowAll() {
    assertThrows(
      () => {
        Deno.readTextFileSync("/proc/net/dev");
      },
      Deno.errors.NotCapable,
      `Requires all access to "/proc/net/dev", run again with the --allow-all flag`,
    );
  },
);

Deno.test(
  { permissions: { sys: ["loadavg"] } },
  function loadavgSuccess() {
    const load = Deno.loadavg();
    assertEquals(load.length, 3);
  },
);

Deno.test({ permissions: { sys: false } }, function loadavgPerm() {
  assertThrows(() => {
    Deno.loadavg();
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { sys: ["hostname"] } },
  function hostnameDir() {
    assertNotEquals(Deno.hostname(), "");
  },
);

Deno.test(
  { permissions: { run: [Deno.execPath()], read: true } },
  // See https://github.com/denoland/deno/issues/16527
  async function hostnameWithoutOtherNetworkUsages() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "-p", "Deno.hostname()"],
      env: {
        LD_PRELOAD: "",
        LD_LIBRARY_PATH: "",
        DYLD_FALLBACK_LIBRARY_PATH: "",
      },
    }).output();
    const hostname = new TextDecoder().decode(stdout).trim();
    assert(hostname.length > 0);
  },
);

Deno.test({ permissions: { sys: false } }, function hostnamePerm() {
  assertThrows(() => {
    Deno.hostname();
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { sys: ["osRelease"] } },
  function releaseDir() {
    assertNotEquals(Deno.osRelease(), "");
  },
);

Deno.test({ permissions: { sys: false } }, function releasePerm() {
  assertThrows(() => {
    Deno.osRelease();
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { sys: ["osUptime"] } }, function osUptime() {
  const uptime = Deno.osUptime();
  assert(typeof uptime === "number");
  assert(uptime > 0);
});

Deno.test({ permissions: { sys: false } }, function osUptimePerm() {
  assertThrows(() => {
    Deno.osUptime();
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { sys: ["systemMemoryInfo"] } },
  function systemMemoryInfo() {
    const info = Deno.systemMemoryInfo();
    assert(info.total >= 0);
    assert(info.free >= 0);
    assert(info.available >= 0);
    assert(info.buffers >= 0);
    assert(info.cached >= 0);
    assert(info.swapTotal >= 0);
    assert(info.swapFree >= 0);
  },
);

Deno.test({ permissions: { sys: ["uid"] } }, function getUid() {
  if (Deno.build.os === "windows") {
    assertEquals(Deno.uid(), null);
  } else {
    const uid = Deno.uid();
    assert(typeof uid === "number");
    assert(uid > 0);
  }
});

Deno.test({ permissions: { sys: ["gid"] } }, function getGid() {
  if (Deno.build.os === "windows") {
    assertEquals(Deno.gid(), null);
  } else {
    const gid = Deno.gid();
    assert(typeof gid === "number");
    assert(gid > 0);
  }
});

Deno.test(function memoryUsage() {
  const mem = Deno.memoryUsage();
  assert(typeof mem.rss === "number");
  assert(typeof mem.heapTotal === "number");
  assert(typeof mem.heapUsed === "number");
  assert(typeof mem.external === "number");
  assert(mem.rss >= mem.heapTotal);
});

Deno.test("Deno.exitCode getter and setter", () => {
  // Initial value is 0
  assertEquals(Deno.exitCode, 0);

  try {
    // Set a new value
    Deno.exitCode = 5;
    assertEquals(Deno.exitCode, 5);
  } finally {
    // Reset to initial value
    Deno.exitCode = 0;
  }

  assertEquals(Deno.exitCode, 0);
});

Deno.test("Setting Deno.exitCode to non-number throws TypeError", () => {
  // Throws on non-number values
  assertThrows(
    () => {
      // @ts-expect-error Testing for runtime error
      Deno.exitCode = "123";
    },
    TypeError,
    "Exit code must be a number, got: 123 (string)",
  );

  // Throws on bigint values
  assertThrows(
    () => {
      // @ts-expect-error Testing for runtime error
      Deno.exitCode = 1n;
    },
    TypeError,
    "Exit code must be a number, got: 1 (bigint)",
  );
});

Deno.test("Setting Deno.exitCode to non-integer throws RangeError", () => {
  // Throws on non-integer values
  assertThrows(
    () => {
      Deno.exitCode = 3.14;
    },
    RangeError,
    "Exit code must be an integer, got: 3.14",
  );
});

Deno.test("Setting Deno.exitCode does not cause an immediate exit", () => {
  let exited = false;

  const originalExit = Deno.exit;
  // @ts-expect-error; read-only
  Deno.exit = () => {
    exited = true;
  };

  try {
    Deno.exitCode = 1;
    assertEquals(exited, false);
  } finally {
    Deno.exit = originalExit;
    Deno.exitCode = 0;
  }
});
