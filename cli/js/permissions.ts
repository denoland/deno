// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch.ts";
import { sendSync } from "./dispatch_json.ts";

/** Permissions as granted by the caller
 * See: https://w3c.github.io/permissions/#permission-registry
 */
export type PermissionName =
  | "read"
  | "write"
  | "net"
  | "env"
  | "run"
  | "hrtime";
// NOTE: Keep in sync with cli/permissions.rs

/** https://w3c.github.io/permissions/#status-of-a-permission */
export type PermissionState = "granted" | "denied" | "prompt";

/** See: https://w3c.github.io/permissions/#permission-descriptor */
interface PermissionDescriptor {
  name: PermissionName;
  url?: string;
  path?: string;
}

/** https://w3c.github.io/permissions/#permissionstatus */
export class PermissionStatus {
  constructor(public state: PermissionState) {}
  // TODO(kt3k): implement onchange handler
}

export class Permissions {
  /** Queries the permission. */
  async query(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = sendSync(dispatch.OP_QUERY_PERMISSION, desc);
    return new PermissionStatus(state);
  }

  /** Revokes the permission. */
  async revoke(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = sendSync(dispatch.OP_REVOKE_PERMISSION, desc);
    return new PermissionStatus(state);
  }
}

export const permissions = new Permissions();
