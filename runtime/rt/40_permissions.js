// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const { illegalConstructorKey } = window.__bootstrap.webUtil;

  function opQuery(desc) {
    return core.jsonOpSync("op_query_permission", desc).state;
  }

  function opRevoke(desc) {
    return core.jsonOpSync("op_revoke_permission", desc).state;
  }

  function opRequest(desc) {
    return core.jsonOpSync("op_request_permission", desc).state;
  }

  class PermissionStatus {
    constructor(state = null, key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      this.state = state;
    }
    // TODO(kt3k): implement onchange handler
  }

  class Permissions {
    constructor(key) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
    }

    query(desc) {
      const state = opQuery(desc);
      return Promise.resolve(
        new PermissionStatus(state, illegalConstructorKey),
      );
    }

    revoke(desc) {
      const state = opRevoke(desc);
      return Promise.resolve(
        new PermissionStatus(state, illegalConstructorKey),
      );
    }

    request(desc) {
      const state = opRequest(desc);
      return Promise.resolve(
        new PermissionStatus(state, illegalConstructorKey),
      );
    }
  }

  const permissions = new Permissions(illegalConstructorKey);

  window.__bootstrap.permissions = {
    permissions,
    Permissions,
    PermissionStatus,
  };
})(this);
