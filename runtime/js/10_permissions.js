// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  op_query_permission,
  op_request_permission,
  op_revoke_permission,
} from "ext:core/ops";
const {
  ArrayIsArray,
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  ArrayPrototypeSlice,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  FunctionPrototypeCall,
  PromiseResolve,
  PromiseReject,
  ReflectHas,
  SafeArrayIterator,
  SafeMap,
  Symbol,
  SymbolFor,
  TypeError,
} = primordials;

import { pathFromURL } from "ext:deno_web/00_infra.js";
import { Event, EventTarget } from "ext:deno_web/02_event.js";

const illegalConstructorKey = Symbol("illegalConstructorKey");

/**
 * @typedef StatusCacheValue
 * @property {PermissionState} state
 * @property {PermissionStatus} status
 * @property {boolean} partial
 */

/** @type {ReadonlyArray<"read" | "write" | "net" | "env" | "sys" | "run" | "ffi">} */
const permissionNames = [
  "read",
  "write",
  "net",
  "env",
  "sys",
  "run",
  "ffi",
];

/**
 * @param {Deno.PermissionDescriptor} desc
 * @returns {Deno.PermissionState}
 */
function opQuery(desc) {
  return op_query_permission(desc);
}

/**
 * @param {Deno.PermissionDescriptor} desc
 * @returns {Deno.PermissionState}
 */
function opRevoke(desc) {
  return op_revoke_permission(desc);
}

/**
 * @param {Deno.PermissionDescriptor} desc
 * @returns {Deno.PermissionState}
 */
function opRequest(desc) {
  return op_request_permission(desc);
}

class PermissionStatus extends EventTarget {
  /** @type {{ state: Deno.PermissionState, partial: boolean }} */
  #status;

  /** @type {((this: PermissionStatus, event: Event) => any) | null} */
  onchange = null;

  /** @returns {Deno.PermissionState} */
  get state() {
    return this.#status.state;
  }

  /** @returns {boolean} */
  get partial() {
    return this.#status.partial;
  }

  /**
   * @param {{ state: Deno.PermissionState, partial: boolean }} status
   * @param {unknown} key
   */
  constructor(status = null, key = null) {
    if (key != illegalConstructorKey) {
      throw new TypeError("Illegal constructor");
    }
    super();
    this.#status = status;
  }

  /**
   * @param {Event} event
   * @returns {boolean}
   */
  dispatchEvent(event) {
    let dispatched = super.dispatchEvent(event);
    if (dispatched && this.onchange) {
      FunctionPrototypeCall(this.onchange, this, event);
      dispatched = !event.defaultPrevented;
    }
    return dispatched;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    const object = { state: this.state, onchange: this.onchange };
    if (this.partial) object.partial = this.partial;
    return `${this.constructor.name} ${inspect(object, inspectOptions)}`;
  }
}

/** @type {Map<string, StatusCacheValue>} */
const statusCache = new SafeMap();

/**
 * @param {Deno.PermissionDescriptor} desc
 * @param {{ state: Deno.PermissionState, partial: boolean }} rawStatus
 * @returns {PermissionStatus}
 */
function cache(desc, rawStatus) {
  let { name: key } = desc;
  if (
    (desc.name === "read" || desc.name === "write" || desc.name === "ffi") &&
    ReflectHas(desc, "path")
  ) {
    key += `-${desc.path}&`;
  } else if (desc.name === "net" && desc.host) {
    key += `-${desc.host}&`;
  } else if (desc.name === "run" && desc.command) {
    key += `-${desc.command}&`;
  } else if (desc.name === "env" && desc.variable) {
    key += `-${desc.variable}&`;
  } else if (desc.name === "sys" && desc.kind) {
    key += `-${desc.kind}&`;
  } else {
    key += "$";
  }
  if (MapPrototypeHas(statusCache, key)) {
    const cachedObj = MapPrototypeGet(statusCache, key);
    if (
      cachedObj.state !== rawStatus.state ||
      cachedObj.partial !== rawStatus.partial
    ) {
      cachedObj.state = rawStatus.state;
      cachedObj.partial = rawStatus.partial;
      cachedObj.status.dispatchEvent(
        new Event("change", { cancelable: false }),
      );
    }
    return cachedObj.status;
  }
  /** @type {{ state: Deno.PermissionState, partial: boolean, status?: PermissionStatus }} */
  const obj = rawStatus;
  obj.status = new PermissionStatus(obj, illegalConstructorKey);
  MapPrototypeSet(statusCache, key, obj);
  return obj.status;
}

/**
 * @param {unknown} desc
 * @returns {desc is Deno.PermissionDescriptor}
 */
function isValidDescriptor(desc) {
  return typeof desc === "object" && desc !== null &&
    ArrayPrototypeIncludes(permissionNames, desc.name);
}

/**
 * @param {Deno.PermissionDescriptor} desc
 * @returns {desc is Deno.PermissionDescriptor}
 */
function formDescriptor(desc) {
  if (
    desc.name === "read" || desc.name === "write" || desc.name === "ffi"
  ) {
    desc.path = pathFromURL(desc.path);
  } else if (desc.name === "run") {
    desc.command = pathFromURL(desc.command);
  }
}

class Permissions {
  constructor(key = null) {
    if (key != illegalConstructorKey) {
      throw new TypeError("Illegal constructor");
    }
  }

  query(desc) {
    try {
      return PromiseResolve(this.querySync(desc));
    } catch (error) {
      return PromiseReject(error);
    }
  }

  querySync(desc) {
    if (!isValidDescriptor(desc)) {
      throw new TypeError(
        `The provided value "${desc?.name}" is not a valid permission name`,
      );
    }

    formDescriptor(desc);

    const status = opQuery(desc);
    return cache(desc, status);
  }

  revoke(desc) {
    try {
      return PromiseResolve(this.revokeSync(desc));
    } catch (error) {
      return PromiseReject(error);
    }
  }

  revokeSync(desc) {
    if (!isValidDescriptor(desc)) {
      throw new TypeError(
        `The provided value "${desc?.name}" is not a valid permission name`,
      );
    }

    formDescriptor(desc);

    const status = opRevoke(desc);
    return cache(desc, status);
  }

  request(desc) {
    try {
      return PromiseResolve(this.requestSync(desc));
    } catch (error) {
      return PromiseReject(error);
    }
  }

  requestSync(desc) {
    if (!isValidDescriptor(desc)) {
      throw new TypeError(
        `The provided value "${desc?.name}" is not a valid permission name.`,
      );
    }

    formDescriptor(desc);

    const status = opRequest(desc);
    return cache(desc, status);
  }
}

const permissions = new Permissions(illegalConstructorKey);

/** Converts all file URLs in FS allowlists to paths. */
function serializePermissions(permissions) {
  if (typeof permissions == "object" && permissions != null) {
    const serializedPermissions = { __proto__: null };
    for (
      const key of new SafeArrayIterator([
        "read",
        "write",
        "run",
        "ffi",
        "import",
      ])
    ) {
      if (ArrayIsArray(permissions[key])) {
        serializedPermissions[key] = ArrayPrototypeMap(
          permissions[key],
          (path) => pathFromURL(path),
        );
      } else {
        serializedPermissions[key] = permissions[key];
      }
    }
    for (
      const key of new SafeArrayIterator(["env", "net", "sys"])
    ) {
      if (ArrayIsArray(permissions[key])) {
        serializedPermissions[key] = ArrayPrototypeSlice(permissions[key]);
      } else {
        serializedPermissions[key] = permissions[key];
      }
    }
    return serializedPermissions;
  }
  return permissions;
}

export { Permissions, permissions, PermissionStatus, serializePermissions };
