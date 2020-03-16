// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as permissionsOps from "./ops/permissions.ts";

export type PermissionName =
  | "read"
  | "write"
  | "net"
  | "env"
  | "run"
  | "plugin"
  | "hrtime";
// NOTE: Keep in sync with cli/permissions.rs

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
type PermissionDescriptor =
  | RunPermissionDescriptor
  | ReadWritePermissionDescriptor
  | NetPermissionDescriptor
  | EnvPermissionDescriptor
  | PluginPermissionDescriptor
  | HrtimePermissionDescriptor;

export class PermissionStatus {
  constructor(public state: PermissionState) {}
  // TODO(kt3k): implement onchange handler
}

export class Permissions {
  async query(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const state = permissionsOps.query(desc);
    return new PermissionStatus(state);
  }

  async revoke(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const state = permissionsOps.revoke(desc);
    return new PermissionStatus(state);
  }

  async request(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const state = permissionsOps.request(desc);
    return new PermissionStatus(state);
  }
}

export const permissions = new Permissions();
