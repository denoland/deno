// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertRejects, assertThrows } from "./test_util.ts";

Deno.test({
  name: "openSyncReadSymlinkToDotDotPermissionDenied",
  fn() {
    assertThrows(() => {
      Deno.openSync(
        "cli/tests/unit/testdata/symlink_to_dot_dot",
        {
          read: true,
        },
      );
    }, Deno.errors.PermissionDenied);
  },
  permissions: {
    read: ["cli/tests/unit/testdata"],
  },
});

Deno.test({
  name: "openReadSymlinkToDotDotPermissionDenied",
  async fn() {
    await assertRejects(async () => {
      await Deno.open(
        "cli/tests/unit/testdata/symlink_to_dot_dot",
        {
          read: true,
        },
      );
    }, Deno.errors.PermissionDenied);
  },
  permissions: {
    read: ["cli/tests/unit/testdata"],
  },
});

Deno.test({
  name: "openReadSymlinkToDotDotPermissionDenied",
  async fn() {
    await assertRejects(async () => {
      await Deno.open(
        "cli/tests/unit/testdata/symlink_to_dot_dot",
        {
          read: true,
        },
      );
    }, Deno.errors.PermissionDenied);
  },
  permissions: {
    read: ["cli/tests/unit/testdata"],
  },
});

Deno.test({
  name: "openSyncWriteSymlinkToDotDotPermissionDenied",
  fn() {
    assertThrows(() => {
      Deno.openSync(
        "cli/tests/unit/testdata/symlink_to_dot_dot",
        {
          write: true,
        },
      );
    }, Deno.errors.PermissionDenied);
  },
  permissions: {
    write: ["cli/tests/unit/testdata"],
  },
});

Deno.test({
  name: "openWriteSymlinkToDotDotPermissionDenied",
  async fn() {
    await assertRejects(async () => {
      await Deno.open(
        "cli/tests/unit/testdata/symlink_to_dot_dot",
        {
          write: true,
        },
      );
    }, Deno.errors.PermissionDenied);
  },
  permissions: {
    write: ["cli/tests/unit/testdata"],
  },
});
