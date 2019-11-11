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

interface RunPermissionDescriptor {
  name: "run";
}
interface ReadWritePermissionDescriptor {
  name: "read" | "write";
  path?: string;
}
interface NetPermissionDescriptor {
  name: "net";
  url?: string;
}
interface EnvPermissionDescriptor {
  name: "env";
}
interface HrtimePermissionDescriptor {
  name: "hrtime";
}
/** See: https://w3c.github.io/permissions/#permission-descriptor */
type PermissionDescriptor =
  | RunPermissionDescriptor
  | ReadWritePermissionDescriptor
  | NetPermissionDescriptor
  | EnvPermissionDescriptor
  | HrtimePermissionDescriptor;

/** https://w3c.github.io/permissions/#permissionstatus */
export class PermissionStatus {
  constructor(public state: PermissionState) {}
  // TODO(kt3k): implement onchange handler
}

export class Permissions {
  /** Queries the permission.
   *       const status = await Deno.permissions.query({ name: "read", path: "/etc" });
   *       if (status.state === "granted") {
   *         file = await Deno.readFile("/etc/passwd");
   *       }
   */
  async query(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = sendSync(dispatch.OP_QUERY_PERMISSION, desc);
    return new PermissionStatus(state);
  }

  /** Revokes the permission.
   *       const status = await Deno.permissions.revoke({ name: "run" });
   *       assert(status.state !== "granted")
   */
  async revoke(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = sendSync(dispatch.OP_REVOKE_PERMISSION, desc);
    return new PermissionStatus(state);
  }

  /** Requests the permission.
   *       const status = await Deno.permissions.request({ name: "env" });
   *       if (status.state === "granted") {
   *         console.log(Deno.homeDir());
   *       } else {
   *         console.log("'env' permission is denied.");
   *       }
   */
  async request(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = sendSync(dispatch.OP_REQUEST_PERMISSION, desc);
    return new PermissionStatus(state);
  }
}

export const permissions = new Permissions();
