// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch.ts";
import { sendSync } from "./dispatch_json.ts";
import * as eventTarget from "./event_target.ts";

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
// NOTE: Keep in sync with src/permissions.rs

/** https://w3c.github.io/permissions/#status-of-a-permission */
export type PermissionState = "granted" | "denied" | "prompt";

/** See: https://w3c.github.io/permissions/#permission-descriptor */
interface PermissionDescriptor {
  name: PermissionName;
}

export class Permissions {
  /**
   */
  async query(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = await sendSync(dispatch.OP_QUERY_PERMISSION, {
      name: desc.name
    });
    return new PermissionStatus(desc, state);
  }

  /**
   */
  async revoke(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = await sendSync(dispatch.OP_REVOKE_PERMISSION, {
      name: desc.name
    });
    return new PermissionStatus(desc, state);
  }

  /**
   */
  async request(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const { state } = await sendSync(dispatch.OP_REQUEST_PERMISSION, {
      name: desc.name
    });
    return new PermissionStatus(desc, state);
  }
}

export const permissions = new Permissions();

/** https://w3c.github.io/permissions/#permissionstatus */
export class PermissionStatus extends eventTarget.EventTarget {
  private _desc: PermissionDescriptor;
  state: PermissionState;
  constructor(desc: PermissionDescriptor, state: PermissionState) {
    super();
    this._desc = desc;
    this.state = state;
  }
  // TODO(kt3k): implement onchange handler
}
