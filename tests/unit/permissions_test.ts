// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";

Deno.test(async function permissionInvalidName() {
  await assertRejects(async () => {
    // deno-lint-ignore no-explicit-any
    await Deno.permissions.query({ name: "foo" as any });
  }, TypeError);
});

Deno.test(function permissionInvalidNameSync() {
  assertThrows(() => {
    // deno-lint-ignore no-explicit-any
    Deno.permissions.querySync({ name: "foo" as any });
  }, TypeError);
});

Deno.test(
  { permissions: { net: [] } },
  async function permissionNetInvalidHost() {
    await assertRejects(async () => {
      await Deno.permissions.query({ name: "net", host: ":" });
    }, URIError);
  },
);

Deno.test(
  { permissions: { net: [] } },
  function permissionNetInvalidHostSync() {
    assertThrows(() => {
      Deno.permissions.querySync({ name: "net", host: ":" });
    }, URIError);
  },
);

Deno.test(
  { permissions: { sys: [] } },
  async function permissionSysValidKind() {
    await Deno.permissions.query({ name: "sys", kind: "loadavg" });
    await Deno.permissions.query({ name: "sys", kind: "osRelease" });
    await Deno.permissions.query({ name: "sys", kind: "osUptime" });
    await Deno.permissions.query({ name: "sys", kind: "networkInterfaces" });
    await Deno.permissions.query({ name: "sys", kind: "systemMemoryInfo" });
    await Deno.permissions.query({ name: "sys", kind: "hostname" });
    await Deno.permissions.query({ name: "sys", kind: "uid" });
    await Deno.permissions.query({ name: "sys", kind: "gid" });
    await Deno.permissions.query({ name: "sys", kind: "cpus" });
  },
);

Deno.test(
  { permissions: { sys: [] } },
  function permissionSysValidKindSync() {
    Deno.permissions.querySync({ name: "sys", kind: "loadavg" });
    Deno.permissions.querySync({ name: "sys", kind: "osRelease" });
    Deno.permissions.querySync({ name: "sys", kind: "networkInterfaces" });
    Deno.permissions.querySync({ name: "sys", kind: "systemMemoryInfo" });
    Deno.permissions.querySync({ name: "sys", kind: "hostname" });
    Deno.permissions.querySync({ name: "sys", kind: "uid" });
    Deno.permissions.querySync({ name: "sys", kind: "gid" });
    Deno.permissions.querySync({ name: "sys", kind: "cpus" });
  },
);

Deno.test(
  { permissions: { sys: [] } },
  async function permissionSysInvalidKind() {
    await assertRejects(async () => {
      // deno-lint-ignore no-explicit-any
      await Deno.permissions.query({ name: "sys", kind: "abc" as any });
    }, TypeError);
  },
);

Deno.test(
  { permissions: { sys: [] } },
  function permissionSysInvalidKindSync() {
    assertThrows(() => {
      // deno-lint-ignore no-explicit-any
      Deno.permissions.querySync({ name: "sys", kind: "abc" as any });
    }, TypeError);
  },
);

Deno.test(async function permissionQueryReturnsEventTarget() {
  const status = await Deno.permissions.query({ name: "read", path: "." });
  assert(["granted", "denied", "prompt"].includes(status.state));
  let called = false;
  status.addEventListener("change", () => {
    called = true;
  });
  status.dispatchEvent(new Event("change"));
  assert(called);
  assert(
    status === (await Deno.permissions.query({ name: "read", path: "." })),
  );
});

Deno.test(function permissionQueryReturnsEventTargetSync() {
  const status = Deno.permissions.querySync({ name: "read", path: "." });
  assert(["granted", "denied", "prompt"].includes(status.state));
  let called = false;
  status.addEventListener("change", () => {
    called = true;
  });
  status.dispatchEvent(new Event("change"));
  assert(called);
  assert(status === Deno.permissions.querySync({ name: "read", path: "." }));
});

Deno.test(async function permissionQueryForReadReturnsSameStatus() {
  const status1 = await Deno.permissions.query({
    name: "read",
    path: ".",
  });
  const status2 = await Deno.permissions.query({
    name: "read",
    path: ".",
  });
  assert(status1 === status2);
});

Deno.test(function permissionQueryForReadReturnsSameStatusSync() {
  const status1 = Deno.permissions.querySync({
    name: "read",
    path: ".",
  });
  const status2 = Deno.permissions.querySync({
    name: "read",
    path: ".",
  });
  assert(status1 === status2);
});

Deno.test(function permissionsIllegalConstructor() {
  assertThrows(() => new Deno.Permissions(), TypeError, "Illegal constructor");
  assertEquals(Deno.Permissions.length, 0);
});

Deno.test(function permissionStatusIllegalConstructor() {
  assertThrows(
    () => new Deno.PermissionStatus(),
    TypeError,
    "Illegal constructor",
  );
  assertEquals(Deno.PermissionStatus.length, 0);
});

// Regression test for https://github.com/denoland/deno/issues/17020
Deno.test(
  { permissions: { read: [], write: [], ffi: [], run: [] } },
  async function permissionURL() {
    const path = new URL(".", import.meta.url);

    await Deno.permissions.query({ name: "read", path });
    await Deno.permissions.query({ name: "write", path });
    await Deno.permissions.query({ name: "ffi", path });
    await Deno.permissions.query({ name: "run", command: path });
  },
);

Deno.test(
  { permissions: { read: [], write: [], ffi: [], run: [] } },
  function permissionURLSync() {
    Deno.permissions.querySync({
      name: "read",
      path: new URL(".", import.meta.url),
    });
    Deno.permissions.querySync({
      name: "write",
      path: new URL(".", import.meta.url),
    });
    Deno.permissions.querySync({
      name: "run",
      command: new URL(".", import.meta.url),
    });
  },
);

Deno.test(async function permissionDescriptorValidation() {
  for (const value of [undefined, null, {}]) {
    for (const method of ["query", "request", "revoke"]) {
      await assertRejects(
        async () => {
          // deno-lint-ignore no-explicit-any
          await (Deno.permissions as any)[method](value as any);
        },
        TypeError,
        '"undefined" is not a valid permission name',
      );
    }
  }
});

Deno.test(function permissionDescriptorValidationSync() {
  for (const value of [undefined, null, {}]) {
    for (const method of ["querySync", "revokeSync", "requestSync"]) {
      assertThrows(
        () => {
          // deno-lint-ignore no-explicit-any
          (Deno.permissions as any)[method](value as any);
        },
        TypeError,
        '"undefined" is not a valid permission name',
      );
    }
  }
});

// Regression test for https://github.com/denoland/deno/issues/15894.
Deno.test(async function permissionStatusObjectsNotEqual() {
  assert(
    await Deno.permissions.query({ name: "env", variable: "A" }) !=
      await Deno.permissions.query({ name: "env", variable: "B" }),
  );
});

Deno.test(function permissionStatusObjectsNotEqualSync() {
  assert(
    Deno.permissions.querySync({ name: "env", variable: "A" }) !=
      Deno.permissions.querySync({ name: "env", variable: "B" }),
  );
});
