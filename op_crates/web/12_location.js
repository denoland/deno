// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { URL } = window.__bootstrap.url;
  const locationConstructorKey = Symbol("locationConstuctorKey");

  class Location {
    constructor(href, key) {
      if (key != locationConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      const url = new URL(href);
      Object.defineProperties(this, {
        hash: {
          get() {
            return url.hash;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.hash".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        host: {
          get() {
            return url.host;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.host".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        hostname: {
          get() {
            return url.hostname;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.hostname".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        href: {
          get() {
            return href;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.href".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        origin: {
          get() {
            return url.origin;
          },
          enumerable: true,
        },
        password: {
          get() {
            return url.password;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.password".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        pathname: {
          get() {
            return url.pathname;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.pathname".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        port: {
          get() {
            return url.port;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.port".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        protocol: {
          get() {
            return url.protocol;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.protocol".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        search: {
          get() {
            return url.search;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.search".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        username: {
          get() {
            return url.username;
          },
          set() {
            throw new DOMException(
              `Cannot set "location.username".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        ancestorOrigins: {
          get() {
            // TODO(nayeemrmn): Replace with a `DOMStringList` instance.
            return {
              length: 0,
              item: () => null,
              contains: () => false,
            };
          },
          enumerable: true,
        },
        assign: {
          value: function assign() {
            throw new DOMException(
              `Cannot call "location.assign()".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        reload: {
          value: function reload() {
            throw new DOMException(
              `Cannot call "location.reload()".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        replace: {
          value: function replace() {
            throw new DOMException(
              `Cannot call "location.replace()".`,
              "NotSupportedError",
            );
          },
          enumerable: true,
        },
        toString: {
          value: function toString() {
            return href;
          },
          enumerable: true,
        },
      });
    }
  }

  Object.defineProperties(Location.prototype, {
    [Symbol.toStringTag]: {
      value: "Location",
      configurable: true,
    },
  });

  let location = null;

  function setLocationHref(href) {
    location = new Location(href, locationConstructorKey);
  }

  window.__bootstrap = (window.__bootstrap || {});
  window.__bootstrap.location = {
    locationConstructorDescriptor: {
      value: Location,
      configurable: true,
      writable: true,
    },
    locationDescriptor: {
      get() {
        if (location == null) {
          throw new ReferenceError(
            `Access to "location", run again with --location <href>.`,
          );
        }
        return location;
      },
      set() {
        throw new DOMException(`Cannot set "location".`, "NotSupportedError");
      },
      enumerable: true,
    },
    setLocationHref,
    getLocationHref() {
      return location?.href;
    },
  };
})(this);
