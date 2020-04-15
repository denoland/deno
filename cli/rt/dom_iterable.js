System.register(
  "$deno$/web/dom_iterable.ts",
  ["$deno$/web/util.ts", "$deno$/internals.ts"],
  function (exports_90, context_90) {
    "use strict";
    let util_ts_14, internals_ts_5;
    const __moduleName = context_90 && context_90.id;
    function DomIterableMixin(Base, dataSymbol) {
      // we have to cast `this` as `any` because there is no way to describe the
      // Base class in a way where the Symbol `dataSymbol` is defined.  So the
      // runtime code works, but we do lose a little bit of type safety.
      // Additionally, we have to not use .keys() nor .values() since the internal
      // slot differs in type - some have a Map, which yields [K, V] in
      // Symbol.iterator, and some have an Array, which yields V, in this case
      // [K, V] too as they are arrays of tuples.
      const DomIterable = class extends Base {
        *entries() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const entry of this[dataSymbol]) {
            yield entry;
          }
        }
        *keys() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const [key] of this[dataSymbol]) {
            yield key;
          }
        }
        *values() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const [, value] of this[dataSymbol]) {
            yield value;
          }
        }
        forEach(
          callbackfn,
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          thisArg
        ) {
          util_ts_14.requiredArguments(
            `${this.constructor.name}.forEach`,
            arguments.length,
            1
          );
          callbackfn = callbackfn.bind(
            thisArg == null ? globalThis : Object(thisArg)
          );
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const [key, value] of this[dataSymbol]) {
            callbackfn(value, key, this);
          }
        }
        *[Symbol.iterator]() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
    exports_90("DomIterableMixin", DomIterableMixin);
    return {
      setters: [
        function (util_ts_14_1) {
          util_ts_14 = util_ts_14_1;
        },
        function (internals_ts_5_1) {
          internals_ts_5 = internals_ts_5_1;
        },
      ],
      execute: function () {
        internals_ts_5.exposeForTest("DomIterableMixin", DomIterableMixin);
      },
    };
  }
);
