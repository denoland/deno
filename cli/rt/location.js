System.register(
  "$deno$/web/location.ts",
  ["$deno$/util.ts", "$deno$/web/dom_util.ts"],
  function (exports_106, context_106) {
    "use strict";
    let util_ts_21, dom_util_ts_1, LocationImpl;
    const __moduleName = context_106 && context_106.id;
    /** Sets the `window.location` at runtime.
     * @internal */
    function setLocation(url) {
      globalThis.location = new LocationImpl(url);
      Object.freeze(globalThis.location);
    }
    exports_106("setLocation", setLocation);
    return {
      setters: [
        function (util_ts_21_1) {
          util_ts_21 = util_ts_21_1;
        },
        function (dom_util_ts_1_1) {
          dom_util_ts_1 = dom_util_ts_1_1;
        },
      ],
      execute: function () {
        LocationImpl = class LocationImpl {
          constructor(url) {
            this.ancestorOrigins = dom_util_ts_1.getDOMStringList([]);
            const u = new URL(url);
            this.#url = u;
            this.hash = u.hash;
            this.host = u.host;
            this.href = u.href;
            this.hostname = u.hostname;
            this.origin = u.protocol + "//" + u.host;
            this.pathname = u.pathname;
            this.protocol = u.protocol;
            this.port = u.port;
            this.search = u.search;
          }
          #url;
          toString() {
            return this.#url.toString();
          }
          assign(_url) {
            throw util_ts_21.notImplemented();
          }
          reload() {
            throw util_ts_21.notImplemented();
          }
          replace(_url) {
            throw util_ts_21.notImplemented();
          }
        };
        exports_106("LocationImpl", LocationImpl);
      },
    };
  }
);
