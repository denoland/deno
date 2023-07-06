// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const ops = core.ops;
import { pathFromURL } from "ext:deno_web/00_infra.js";
import { Event, EventTarget } from "ext:deno_web/02_event.js";
const primordials = globalThis.__bootstrap.primordials;
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

const illegalConstructorKey = Symbol("illegalConstructorKey");

/**
 * @typedef StatusCacheValue
 * @property {PermissionState} state
 * @property {PermissionPartial} partial
 * @property {PermissionStatus} status
 */

/** @type {ReadonlyArray<"read" | "write" | "net" | "env" | "sys" | "run" | "ffi" | "hrtime">} */
const permissionNames = [
  "read",
  "write",
  "net",
  "env",
  "sys",
  "run",
  "ffi",
  "hrtime",
];

/**
 * @param {Deno.PermissionDescriptor} desc
 * @returns {Deno.PermissionState}
 */
function opQuery(desc) {
  return ops.op_query_permission(desc);
}

/**
 * @param {Deno.PermissionDescriptor} desc
 * @returns {Deno.PermissionState}
 */
function opRevoke(desc) {
  return ops.op_revoke_permission(desc);
}

/**
 * @param {Deno.PermissionDescriptor} desc
 * @returns {Deno.PermissionState}
 */
function opRequest(desc) {
  return ops.op_request_permission(desc);
}

class PermissionStatus extends EventTarget {
  /** @type {{ state: Deno.PermissionState, partial: Deno.PermissionPartial }} */
  #status;

  /** @type {((this: PermissionStatus, event: Event) => any) | null} */
  onchange = null;

  /** @returns {Deno.PermissionState} */
  get state() {
    return this.#status.state;
  }

  /** @returns {Deno.PermissionPartial} */
  get partial() {
    return this.#status.partial;
  }

  /**
   * @param {{ state: Deno.PermissionState, partial: Deno.PermissionPartial }} status
   * @param {unknown} key
   */
  constructor(status = null, key = null) {
    if (key != illegalConstructorKey) {
      throw new TypeError("Illegal constructor.");
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

  [SymbolFor("Deno.privateCustomInspect")](inspect) {
    const object = { state: this.state, onchange: this.onchange };
    if (this.partial) object.partial = this.partial;
    return `${this.constructor.name} ${inspect(object)}`;
  }
}

/** @type {Map<string, StatusCacheValue>} */
const statusCache = new SafeMap();

/**
 * @param {Deno.PermissionDescriptor} desc
 * @param {{ state: Deno.PermissionState, partial: Deno.PermissionPartial }} status
 * @returns {PermissionStatus}
 */
function cache(desc, status) {
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
    const cachedStatus = MapPrototypeGet(statusCache, key);
    if (
      cachedStatus.state !== status.state ||
      cachedStatus.partial !== status.partial
    ) {
      cachedStatus.state = status.state;
      cachedStatus.partial = status.partial;
      cachedStatus.status.dispatchEvent(
        new Event("change", { cancelable: false }),
      );
    }
    return cachedStatus;
  }
  status = new PermissionStatus(status, illegalConstructorKey);
  MapPrototypeSet(statusCache, key, status);
  return status;
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
      throw new TypeError("Illegal constructor.");
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
        `The provided value "${desc?.name}" is not a valid permission name.`,
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
        `The provided value "${desc?.name}" is not a valid permission name.`,
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
    const serializedPermissions = {};
    for (
      const key of new SafeArrayIterator(["read", "write", "run", "ffi"])
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
      const key of new SafeArrayIterator(["env", "hrtime", "net", "sys"])
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
