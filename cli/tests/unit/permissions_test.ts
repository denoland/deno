// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

unitTest(async function asyncPermissionsInvalidTests(): Promise<void> {
  // invalid permission names
  await assertAsyncInvalidPermissionHandled(Deno.permissions.query, {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    name: "foo" as any,
  });
  await assertAsyncInvalidPermissionHandled(Deno.permissions.request, {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    name: "foo" as any,
  });
  await assertAsyncInvalidPermissionHandled(Deno.permissions.revoke, {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    name: "foo" as any,
  });

  // invalid net urls
  await assertAsyncInvalidPermissionHandled(Deno.permissions.query, {
    name: "net",
    url: ":",
  });
  await assertAsyncInvalidPermissionHandled(Deno.permissions.request, {
    name: "net",
    url: ":",
  });
  await assertAsyncInvalidPermissionHandled(Deno.permissions.revoke, {
    name: "net",
    url: ":",
  });
});

unitTest(function syncPermissionsInvalidTests(): void {
  // invalid permission names
  assertSyncInvalidPermissionHandled(Deno.permissions.querySync, {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    name: "foo" as any,
  });
  assertSyncInvalidPermissionHandled(Deno.permissions.requestSync, {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    name: "foo" as any,
  });
  assertSyncInvalidPermissionHandled(Deno.permissions.revokeSync, {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    name: "foo" as any,
  });

  // invalid net urls
  assertSyncInvalidPermissionHandled(Deno.permissions.querySync, {
    name: "net",
    url: ":",
  });
  assertSyncInvalidPermissionHandled(Deno.permissions.requestSync, {
    name: "net",
    url: ":",
  });
  assertSyncInvalidPermissionHandled(Deno.permissions.revokeSync, {
    name: "net",
    url: ":",
  });
});

async function assertAsyncInvalidPermissionHandled(
  fn: (desc: Deno.PermissionDescriptor) => Promise<Deno.PermissionStatus>,
  descriptor: Deno.PermissionDescriptor,
): Promise<void> {
  let thrown = false;
  try {
    await fn(descriptor);
  } catch (e) {
    thrown = true;
    assert(e instanceof Error);
  } finally {
    assert(thrown);
  }
}

function assertSyncInvalidPermissionHandled(
  fn: (desc: Deno.PermissionDescriptor) => Deno.PermissionStatus,
  descriptor: Deno.PermissionDescriptor,
): void {
  let thrown = false;
  try {
    fn(descriptor);
  } catch (e) {
    thrown = true;
    assert(e instanceof Error);
  } finally {
    assert(thrown);
  }
}
