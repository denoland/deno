// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { DomIterable } from "./dom_types.ts";
import { requiredArguments } from "./util.ts";
import { exposeForTest } from "../internals.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Constructor<T = {}> = new (...args: any[]) => T;

export function DomIterableMixin<K, V, TBase extends Constructor>(
  Base: TBase,
  dataSymbol: symbol
): TBase & Constructor<DomIterable<K, V>> {
  // we have to cast `this` as `any` because there is no way to describe the
  // Base class in a way where the Symbol `dataSymbol` is defined.  So the
  // runtime code works, but we do lose a little bit of type safety.

  // Additionally, we have to not use .keys() nor .values() since the internal
  // slot differs in type - some have a Map, which yields [K, V] in
  // Symbol.iterator, and some have an Array, which yields V, in this case
  // [K, V] too as they are arrays of tuples.

  const DomIterable = class extends Base {
    *entries(): IterableIterator<[K, V]> {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      for (const entry of (this as any)[dataSymbol]) {
        yield entry;
      }
    }

    *keys(): IterableIterator<K> {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      for (const [key] of (this as any)[dataSymbol]) {
        yield key;
      }
    }

    *values(): IterableIterator<V> {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      for (const [, value] of (this as any)[dataSymbol]) {
        yield value;
      }
    }

    forEach(
      callbackfn: (value: V, key: K, parent: this) => void,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      thisArg?: any
    ): void {
      requiredArguments(
        `${this.constructor.name}.forEach`,
        arguments.length,
        1
      );
      callbackfn = callbackfn.bind(
        thisArg == null ? globalThis : Object(thisArg)
      );
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      for (const [key, value] of (this as any)[dataSymbol]) {
        callbackfn(value, key, this);
      }
    }

    *[Symbol.iterator](): IterableIterator<[K, V]> {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      for (const entry of (this as any)[dataSymbol]) {
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

exposeForTest("DomIterableMixin", DomIterableMixin);
