System.register(
  "$deno$/permissions.ts",
  ["$deno$/ops/permissions.ts"],
  function (exports_48, context_48) {
    "use strict";
    let permissionsOps, PermissionStatus, Permissions;
    const __moduleName = context_48 && context_48.id;
    return {
      setters: [
        function (permissionsOps_1) {
          permissionsOps = permissionsOps_1;
        },
      ],
      execute: function () {
        PermissionStatus = class PermissionStatus {
          constructor(state) {
            this.state = state;
          }
        };
        exports_48("PermissionStatus", PermissionStatus);
        Permissions = class Permissions {
          query(desc) {
            const state = permissionsOps.query(desc);
            return Promise.resolve(new PermissionStatus(state));
          }
          revoke(desc) {
            const state = permissionsOps.revoke(desc);
            return Promise.resolve(new PermissionStatus(state));
          }
          request(desc) {
            const state = permissionsOps.request(desc);
            return Promise.resolve(new PermissionStatus(state));
          }
        };
        exports_48("Permissions", Permissions);
        exports_48("permissions", new Permissions());
      },
    };
  }
);
