// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch";
import { sendSync } from "./dispatch_json";

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

/** Inspect granted permissions for the current program.
 *
 *       if (Deno.permissions().read) {
 *         const file = await Deno.readFile("example.test");
 *         // ...
 *       }
 */
export function permissions(): Permissions {
  return sendSync(dispatch.OP_PERMISSIONS) as Permissions;
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
  sendSync(dispatch.OP_REVOKE_PERMISSION, { permission });
}
