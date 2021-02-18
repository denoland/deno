// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { requiredArguments } = window.__bootstrap.fetchUtil;
  // const { exposeForTest } = window.__bootstrap.internals;

  function DomIterableMixin(
    Base,
    dataSymbol,
  ) {
    // we have to cast `this` as `any` because there is no way to describe the
    // Base class in a way where the Symbol `dataSymbol` is defined.  So the
    // runtime code works, but we do lose a little bit of type safety.

    // Additionally, we have to not use .keys() nor .values() since the internal
    // slot differs in type - some have a Map, which yields [K, V] in
    // Symbol.iterator, and some have an Array, which yields V, in this case
    // [K, V] too as they are arrays of tuples.

    const DomIterable = class extends Base {
      *entries() {
        for (const entry of this[dataSymbol]) {
          yield entry;
        }
      }

      *keys() {
        for (const [key] of this[dataSymbol]) {
          yield key;
        }
      }

      *values() {
        for (const [, value] of this[dataSymbol]) {
          yield value;
        }
      }

      forEach(
        callbackfn,
        thisArg,
      ) {
        requiredArguments(
          `${this.constructor.name}.forEach`,
          arguments.length,
          1,
        );
        callbackfn = callbackfn.bind(
          thisArg == null ? globalThis : Object(thisArg),
        );
        for (const [key, value] of this[dataSymbol]) {
          callbackfn(value, key, this);
        }
      }

      *[Symbol.iterator]() {
        for (const entry of this[dataSymbol]) {
          yield entry;
        }
      }
    };

    // we want the Base class name to be the name of the class.
    Object.defineProperty(DomIterable, "name", {
      value: Base.name,
      configurable: true,
    });

    return DomIterable;
  }

  // exposeForTest("DomIterableMixin", DomIterableMixin);

  window.__bootstrap.domIterable = {
    DomIterableMixin,
  };
})(this);
