// Copyright 2018-2026 the Deno authors. MIT license.
import process from "node:process";
import { assertEquals, assertThrows } from "./test_util.ts";

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { sys: ["umask"] },
  },
  function umaskSuccess() {
    const prevMask = Deno.umask(0o020);
    const newMask = Deno.umask(prevMask);
    const finalMask = Deno.umask();
    assertEquals(newMask, 0o020);
    assertEquals(finalMask, prevMask);
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { sys: false },
  },
  function umaskRequiresSysPermission() {
    assertThrows(() => {
      Deno.umask();
    }, Deno.errors.NotCapable);
    assertThrows(() => {
      Deno.umask(0);
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { sys: ["umask"] },
  },
  function processUmaskSuccess() {
    const initialMask = process.umask();
    const previousMask = process.umask(0o027);
    const changedMask = process.umask(initialMask);
    assertEquals(previousMask, initialMask);
    assertEquals(changedMask, 0o027);
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { sys: false },
  },
  function processUmaskRequiresSysPermission() {
    assertThrows(() => {
      process.umask();
    }, Deno.errors.NotCapable);
    assertThrows(() => {
      process.umask(0);
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { sys: ["umask"] },
  },
  async function workerWithoutSysPermissionCannotUseUmask() {
    const prevMask = Deno.umask(0o022);
    const workerUrl = URL.createObjectURL(
      new Blob([
        `
const results = [];
try {
  Deno.umask();
  results.push("read");
} catch (err) {
  results.push(
    err instanceof Deno.errors.NotCapable
      ? "read NotCapable"
      : err instanceof Error
      ? err.name
      : "unknown",
  );
}
try {
  Deno.umask(0);
  results.push("changed");
} catch (err) {
  results.push(
    err instanceof Deno.errors.NotCapable
      ? "set NotCapable"
      : err instanceof Error
      ? err.name
      : "unknown",
  );
}
postMessage(results);
`,
      ], { type: "application/typescript" }),
    );
    const worker = new Worker(workerUrl, {
      type: "module",
      deno: { permissions: "none" },
    });

    try {
      const result = await new Promise((resolve) => {
        worker.onmessage = (event) => resolve(event.data);
      });
      assertEquals(result, ["read NotCapable", "set NotCapable"]);
      assertEquals(Deno.umask(), 0o022);
    } finally {
      worker.terminate();
      URL.revokeObjectURL(workerUrl);
      Deno.umask(prevMask);
    }
  },
);
