// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as permissionsOps from "../ops/permissions.ts";
import { EventTargetImpl as EventTarget } from "./event_target.ts";

const permissionNames = [
  "read",
  "write",
  "net",
  "env",
  "run",
  "plugin",
  "hrtime",
] as const;

type PermissionName = typeof permissionNames[number];

interface RunPermissionDescriptor {
  name: "run";
}

interface ReadPermissionDescriptor {
  name: "read";
  path?: string;
}

interface WritePermissionDescriptor {
  name: "write";
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

type DenoPermissionDescriptor =
  | RunPermissionDescriptor
  | ReadPermissionDescriptor
  | WritePermissionDescriptor
  | NetPermissionDescriptor
  | EnvPermissionDescriptor
  | PluginPermissionDescriptor
  | HrtimePermissionDescriptor;

interface StatusCacheValue {
  state: PermissionState;
  status: PermissionStatusImpl;
}

export class PermissionStatusImpl extends EventTarget
  implements PermissionStatus {
  #state: { state: PermissionState };

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  onchange: ((this: PermissionStatus, event: Event) => any) | null = null;

  get state(): PermissionState {
    return this.#state.state;
  }

  constructor(state: { state: PermissionState }) {
    super();
    this.#state = state;
  }

  dispatchEvent(event: Event): boolean {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let dispatched = super.dispatchEvent(event as any);
    if (dispatched && this.onchange) {
      this.onchange.call(this, event);
      dispatched = !event.defaultPrevented;
    }
    return dispatched;
  }

  get [Symbol.toStringTag](): string {
    return "PermissionStatus";
  }
}

/** A cache of `PermissionStatus` objects and their last known state. */
const statusCache = new Map<string, StatusCacheValue>();

/** Cache the state of a descriptor and return its `PermissionStatus`. */
function cache(
  desc: DenoPermissionDescriptor,
  state: PermissionState
): PermissionStatusImpl {
  let key = desc.name;
  if ((desc.name === "read" || desc.name === "write") && desc.path) {
    key += `-${desc.path}`;
  } else if (desc.name === "net" && desc.url) {
    key += `-${desc.url}`;
  }
  if (statusCache.has(key)) {
    const status = statusCache.get(key)!;
    if (status.state !== state) {
      status.state = state;
      status.status.dispatchEvent(new Event("change", { cancelable: false }));
    }
    return status.status;
  }
  const status: { state: PermissionState; status?: PermissionStatusImpl } = {
    state,
  };
  status.status = new PermissionStatusImpl(status);
  statusCache.set(key, status as StatusCacheValue);
  return status.status;
}

function isValidDescriptor(
  desc: PermissionDescriptor | DenoPermissionDescriptor
): desc is DenoPermissionDescriptor {
  return permissionNames.includes(desc.name as PermissionName);
}

export class PermissionsImpl implements Permissions {
  query(
    desc: PermissionDescriptor | DenoPermissionDescriptor
  ): Promise<PermissionStatus> {
    if (!isValidDescriptor(desc)) {
      return Promise.reject(
        new TypeError(
          `The provided value "${desc.name}" is not a valid permission name.`
        )
      );
    }
    const state = permissionsOps.query(desc);
    return Promise.resolve(cache(desc, state) as PermissionStatus);
  }

  revoke(
    desc: PermissionDescriptor | DenoPermissionDescriptor
  ): Promise<PermissionStatus> {
    if (!isValidDescriptor(desc)) {
      return Promise.reject(
        new TypeError(
          `The provided value "${desc.name}" is not a valid permission name.`
        )
      );
    }
    const state = permissionsOps.revoke(desc);
    return Promise.resolve(cache(desc, state) as PermissionStatus);
  }

  request(
    desc: PermissionDescriptor | DenoPermissionDescriptor
  ): Promise<PermissionStatus> {
    if (!isValidDescriptor(desc)) {
      return Promise.reject(
        new TypeError(
          `The provided value "${desc.name}" is not a valid permission name.`
        )
      );
    }
    const state = permissionsOps.request(desc);
    return Promise.resolve(cache(desc, state) as PermissionStatus);
  }
}

Object.defineProperty(PermissionStatusImpl, "name", {
  value: "PermissionStatus",
  configurable: true,
});
Object.defineProperty(PermissionsImpl, "name", {
  value: "Permissions",
  configurable: true,
});
