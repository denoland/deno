// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./ops/dispatch_json.ts";

/** Permissions as granted by the caller
 * See: https://w3c.github.io/permissions/#permission-registry
 */
export type PermissionName =
  | "read"
  | "write"
  | "net"
  | "env"
  | "run"
  | "plugin"
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
interface PluginPermissionDescriptor {
  name: "plugin";
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
  | PluginPermissionDescriptor
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
    const { state } = sendSync("op_query_permission", desc);
    return new PermissionStatus(state);
  }

  /** Revokes the permission.
   *       const status = await Deno.permissions.revoke({ name: "run" });
   *       assert(status.state !== "granted")
   */
  async revoke(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = sendSync("op_revoke_permission", desc);
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
    const { state } = sendSync("op_request_permission", desc);
    return new PermissionStatus(state);
  }
}

export const permissions = new Permissions();
