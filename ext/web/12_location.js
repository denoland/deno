// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check

import { primordials } from "ext:core/mod.js";
const {
  Error,
  ObjectDefineProperties,
  SafeWeakMap,
  Symbol,
  SymbolFor,
  SymbolToStringTag,
  TypeError,
  WeakMapPrototypeGet,
  WeakMapPrototypeSet,
} = primordials;

import { URL } from "ext:deno_url/00_url.js";
import { DOMException } from "./01_dom_exception.js";

const locationConstructorKey = Symbol("locationConstructorKey");

// The differences between the definitions of `Location` and `WorkerLocation`
// are because of the `LegacyUnforgeable` attribute only specified upon
// `Location`'s properties. See:
// - https://html.spec.whatwg.org/multipage/history.html#the-location-interface
// - https://heycam.github.io/webidl/#LegacyUnforgeable
class Location {
  constructor(href = null, key = null) {
    if (key != locationConstructorKey) {
      throw new TypeError("Illegal constructor.");
    }
    const url = new URL(href);
    url.username = "";
    url.password = "";
    ObjectDefineProperties(this, {
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
          return url.href;
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
          return url.href;
        },
        enumerable: true,
      },
      [SymbolFor("Deno.privateCustomInspect")]: {
        value: function (inspect, inspectOptions) {
          return `${this.constructor.name} ${
            inspect({
              hash: this.hash,
              host: this.host,
              hostname: this.hostname,
              href: this.href,
              origin: this.origin,
              pathname: this.pathname,
              port: this.port,
              protocol: this.protocol,
              search: this.search,
            }, inspectOptions)
          }`;
        },
      },
    });
  }
}

ObjectDefineProperties(Location.prototype, {
  [SymbolToStringTag]: {
    value: "Location",
    configurable: true,
  },
});

const workerLocationUrls = new SafeWeakMap();

class WorkerLocation {
  constructor(href = null, key = null) {
    if (key != locationConstructorKey) {
      throw new TypeError("Illegal constructor.");
    }
    const url = new URL(href);
    url.username = "";
    url.password = "";
    WeakMapPrototypeSet(workerLocationUrls, this, url);
  }
}

ObjectDefineProperties(WorkerLocation.prototype, {
  hash: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.hash;
    },
    configurable: true,
    enumerable: true,
  },
  host: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.host;
    },
    configurable: true,
    enumerable: true,
  },
  hostname: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.hostname;
    },
    configurable: true,
    enumerable: true,
  },
  href: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.href;
    },
    configurable: true,
    enumerable: true,
  },
  origin: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.origin;
    },
    configurable: true,
    enumerable: true,
  },
  pathname: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.pathname;
    },
    configurable: true,
    enumerable: true,
  },
  port: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.port;
    },
    configurable: true,
    enumerable: true,
  },
  protocol: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.protocol;
    },
    configurable: true,
    enumerable: true,
  },
  search: {
    get() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.search;
    },
    configurable: true,
    enumerable: true,
  },
  toString: {
    value: function toString() {
      const url = WeakMapPrototypeGet(workerLocationUrls, this);
      if (url == null) {
        throw new TypeError("Illegal invocation.");
      }
      return url.href;
    },
    configurable: true,
    enumerable: true,
    writable: true,
  },
  [SymbolToStringTag]: {
    value: "WorkerLocation",
    configurable: true,
  },
  [SymbolFor("Deno.privateCustomInspect")]: {
    value: function (inspect, inspectOptions) {
      return `${this.constructor.name} ${
        inspect({
          hash: this.hash,
          host: this.host,
          hostname: this.hostname,
          href: this.href,
          origin: this.origin,
          pathname: this.pathname,
          port: this.port,
          protocol: this.protocol,
          search: this.search,
        }, inspectOptions)
      }`;
    },
  },
});

let location = undefined;
let workerLocation = undefined;

function setLocationHref(href) {
  location = new Location(href, locationConstructorKey);
  workerLocation = new WorkerLocation(href, locationConstructorKey);
}

function getLocationHref() {
  return location?.href;
}

const locationConstructorDescriptor = {
  value: Location,
  configurable: true,
  writable: true,
};

const workerLocationConstructorDescriptor = {
  value: WorkerLocation,
  configurable: true,
  writable: true,
};

const locationDescriptor = {
  get() {
    return location;
  },
  set() {
    throw new DOMException(`Cannot set "location".`, "NotSupportedError");
  },
  enumerable: true,
};
const workerLocationDescriptor = {
  get() {
    if (workerLocation == null) {
      throw new Error(
        `Assertion: "globalThis.location" must be defined in a worker.`,
      );
    }
    return workerLocation;
  },
  configurable: true,
  enumerable: true,
};

export {
  getLocationHref,
  locationConstructorDescriptor,
  locationDescriptor,
  setLocationHref,
  workerLocationConstructorDescriptor,
  workerLocationDescriptor,
};
