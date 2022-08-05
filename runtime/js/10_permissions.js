// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    Event,
    EventTarget,
    Deno: { core },
    __bootstrap: { webUtil: { illegalConstructorKey } },
  } = window;
  const { pathFromURL } = window.__bootstrap.util;
  const {
    ArrayIsArray,
    ArrayPrototypeIncludes,
    ArrayPrototypeMap,
    ArrayPrototypeSlice,
    Map,
    MapPrototypeGet,
    MapPrototypeHas,
    MapPrototypeSet,
    FunctionPrototypeCall,
    PromiseResolve,
    PromiseReject,
    ReflectHas,
    SymbolFor,
    TypeError,
  } = window.__bootstrap.primordials;

  /**
   * @typedef StatusCacheValue
   * @property {PermissionState} state
   * @property {PermissionStatus} status
   */

  /** @type {ReadonlyArray<"read" | "write" | "net" | "env" | "run" | "ffi" | "hrtime">} */
  const permissionNames = [
    "read",
    "write",
    "net",
    "env",
    "run",
    "ffi",
    "hrtime",
  ];

  /**
   * @param {Deno.PermissionDescriptor} desc
   * @returns {Deno.PermissionState}
   */
  function opQuery(desc) {
    return core.opSync("op_query_permission", desc);
  }

  /**
   * @param {Deno.PermissionDescriptor} desc
   * @returns {Deno.PermissionState}
   */
  function opRevoke(desc) {
    return core.opSync("op_revoke_permission", desc);
  }

  /**
   * @param {Deno.PermissionDescriptor} desc
   * @returns {Deno.PermissionState}
   */
  function opRequest(desc) {
    return core.opSync("op_request_permission", desc);
  }

  class PermissionStatus extends EventTarget {
    /** @type {{ state: Deno.PermissionState }} */
    #state;

    /** @type {((this: PermissionStatus, event: Event) => any) | null} */
    onchange = null;

    /** @returns {Deno.PermissionState} */
    get state() {
      return this.#state.state;
    }

    /**
     * @param {{ state: Deno.PermissionState }} state
     * @param {unknown} key
     */
    constructor(state = null, key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
      this.#state = state;
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
      return `${this.constructor.name} ${
        inspect({ state: this.state, onchange: this.onchange })
      }`;
    }
  }

  /** @type {Map<string, StatusCacheValue>} */
  const statusCache = new Map();

  /**
   * @param {Deno.PermissionDescriptor} desc
   * @param {Deno.PermissionState} state
   * @returns {PermissionStatus}
   */
  function cache(desc, state) {
    let { name: key } = desc;
    if (
      (desc.name === "read" || desc.name === "write") &&
      ReflectHas(desc, "path")
    ) {
      key += `-${desc.path}`;
    } else if (desc.name === "net" && desc.host) {
      key += `-${desc.host}`;
    }
    if (MapPrototypeHas(statusCache, key)) {
      const status = MapPrototypeGet(statusCache, key);
      if (status.state !== state) {
        status.state = state;
        status.status.dispatchEvent(new Event("change", { cancelable: false }));
      }
      return status.status;
    }
    /** @type {{ state: Deno.PermissionState; status?: PermissionStatus }} */
    const status = { state };
    status.status = new PermissionStatus(status, illegalConstructorKey);
    MapPrototypeSet(statusCache, key, status);
    return status.status;
  }

  /**
   * @param {unknown} desc
   * @returns {desc is Deno.PermissionDescriptor}
   */
  function isValidDescriptor(desc) {
    return typeof desc === "object" && desc !== null &&
      ArrayPrototypeIncludes(permissionNames, desc.name);
  }

  class Permissions {
    constructor(key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
    }

    query(desc) {
      if (!isValidDescriptor(desc)) {
        return PromiseReject(
          new TypeError(
            `The provided value "${desc?.name}" is not a valid permission name.`,
          ),
        );
      }

      if (
        desc.name === "read" || desc.name === "write" || desc.name === "ffi"
      ) {
        desc.path = pathFromURL(desc.path);
      } else if (desc.name === "run") {
        desc.command = pathFromURL(desc.command);
      }

      const state = opQuery(desc);
      return PromiseResolve(cache(desc, state));
    }

    revoke(desc) {
      if (!isValidDescriptor(desc)) {
        return PromiseReject(
          new TypeError(
            `The provided value "${desc?.name}" is not a valid permission name.`,
          ),
        );
      }

      if (desc.name === "read" || desc.name === "write") {
        desc.path = pathFromURL(desc.path);
      } else if (desc.name === "run") {
        desc.command = pathFromURL(desc.command);
      }

      const state = opRevoke(desc);
      return PromiseResolve(cache(desc, state));
    }

    request(desc) {
      if (!isValidDescriptor(desc)) {
        return PromiseReject(
          new TypeError(
            `The provided value "${desc?.name}" is not a valid permission name.`,
          ),
        );
      }

      if (desc.name === "read" || desc.name === "write") {
        desc.path = pathFromURL(desc.path);
      } else if (desc.name === "run") {
        desc.command = pathFromURL(desc.command);
      }

      const state = opRequest(desc);
      return PromiseResolve(cache(desc, state));
    }
  }

  const permissions = new Permissions(illegalConstructorKey);

  /** Converts all file URLs in FS allowlists to paths. */
  function serializePermissions(permissions) {
    if (typeof permissions == "object" && permissions != null) {
      const serializedPermissions = {};
      for (const key of ["read", "write", "run", "ffi"]) {
        if (ArrayIsArray(permissions[key])) {
          serializedPermissions[key] = ArrayPrototypeMap(
            permissions[key],
            (path) => pathFromURL(path),
          );
        } else {
          serializedPermissions[key] = permissions[key];
        }
      }
      for (const key of ["env", "hrtime", "net"]) {
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

  window.__bootstrap.permissions = {
    serializePermissions,
    permissions,
    Permissions,
    PermissionStatus,
  };
})(this);
