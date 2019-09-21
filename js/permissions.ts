// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

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

const OP_PERMISSIONS = new JsonOp("permissions");

/** Inspect granted permissions for the current program.
 *
 *       if (Deno.permissions().read) {
 *         const file = await Deno.readFile("example.test");
 *         // ...
 *       }
 */
export function permissions(): Permissions {
  return OP_PERMISSIONS.sendSync() as Permissions;
}

const OP_REVOKE_PERMISSION = new JsonOp("revoke_permission");

/** Revoke a permission. When the permission was already revoked nothing changes
 *
 *       if (Deno.permissions().read) {
 *         const file = await Deno.readFile("example.test");
 *         Deno.revokePermission('read');
 *       }
 *       Deno.readFile("example.test"); // -> error or permission prompt
 */
export function revokePermission(permission: Permission): void {
  return OP_REVOKE_PERMISSION.sendSync({ permission });
}
