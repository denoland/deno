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

export interface RunPermissionDescriptor {
  name: "run";
}

export interface ReadPermissionDescriptor {
  name: "read";
  path?: string;
}

export interface WritePermissionDescriptor {
  name: "write";
  path?: string;
}

export interface NetPermissionDescriptor {
  name: "net";
  url?: string;
}

export interface EnvPermissionDescriptor {
  name: "env";
}

export interface PluginPermissionDescriptor {
  name: "plugin";
}

export interface HrtimePermissionDescriptor {
  name: "hrtime";
}

export type PermissionDescriptor =
  | RunPermissionDescriptor
  | ReadPermissionDescriptor
  | WritePermissionDescriptor
  | NetPermissionDescriptor
  | EnvPermissionDescriptor
  | PluginPermissionDescriptor
  | HrtimePermissionDescriptor;

export class PermissionStatus {
  constructor(public state: PermissionState) {}
  // TODO(kt3k): implement onchange handler
}

export class Permissions {
  query(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const state = permissionsOps.query(desc);
    return Promise.resolve(new PermissionStatus(state));
  }

  revoke(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const state = permissionsOps.revoke(desc);
    return Promise.resolve(new PermissionStatus(state));
  }

  request(desc: PermissionDescriptor): Promise<PermissionStatus> {
    const state = permissionsOps.request(desc);
    return Promise.resolve(new PermissionStatus(state));
  }
}

export const permissions = new Permissions();
