// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { assert } from "./util";

/** Permissions as granted by the caller */
export interface Permissions {
  read: boolean;
  write: boolean;
  net: boolean;
  env: boolean;
  run: boolean;
  hrtime: boolean;
  // NOTE: Keep in sync with src/permissions.rs
}

export type Permission = keyof Permissions;

function getReq(): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Permissions.createPermissions(builder);
  return [builder, msg.Any.Permissions, inner];
}

function createPermissions(inner: msg.PermissionsRes): Permissions {
  return {
    read: inner.read(),
    write: inner.write(),
    net: inner.net(),
    env: inner.env(),
    run: inner.run(),
    hrtime: inner.hrtime()
  };
}

/** Inspect granted permissions for the current program.
 *
 *       if (Deno.permissions().read) {
 *         const file = await Deno.readFile("example.test");
 *         // ...
 *       }
 */
export function permissions(): Permissions {
  const baseRes = dispatch.sendSync(...getReq())!;
  assert(msg.Any.PermissionsRes === baseRes.innerType());
  const res = new msg.PermissionsRes();
  assert(baseRes.inner(res) != null);
  // TypeScript cannot track assertion above, therefore not null assertion
  return createPermissions(res);
}

function revokeReq(
  permission: string
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const permission_ = builder.createString(permission);
  const inner = msg.PermissionRevoke.createPermissionRevoke(
    builder,
    permission_
  );
  return [builder, msg.Any.PermissionRevoke, inner];
}

/** Revoke a permission. When the permission was already revoked nothing changes
 *
 *       if (Deno.permissions().read) {
 *         const file = await Deno.readFile("example.test");
 *         Deno.revokePermission('read');
 *       }
 *       Deno.readFile("example.test"); // -> error or permission prompt
 */
export function revokePermission(permission: Permission): void {
  dispatch.sendSync(...revokeReq(permission));
}
