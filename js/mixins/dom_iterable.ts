import { DomIterable } from "../dom_types";
import { globalEval } from "../global_eval";
import { requiredArguments } from "../util";

// if we import it directly from "globals" it will break the unit tests so we
// have to grab a reference to the global scope a different way
const window = globalEval("this");

// tslint:disable:no-any
type Constructor<T = {}> = new (...args: any[]) => T;

/** Mixes in a DOM iterable methods into a base class, assumes that there is
 * a private data iterable that is part of the base class, located at
 * `[dataSymbol]`.
 * TODO Don't expose DomIterableMixin from "deno" namespace.
 */
export function DomIterableMixin<K, V, TBase extends Constructor>(
  // tslint:disable-next-line:variable-name
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

  // tslint:disable-next-line:variable-name
  const DomIterable = class extends Base {
    *entries(): IterableIterator<[K, V]> {
      for (const entry of (this as any)[dataSymbol]) {
        yield entry;
      }
    }

    *keys(): IterableIterator<K> {
      for (const [key] of (this as any)[dataSymbol]) {
        yield key;
      }
    }

    *values(): IterableIterator<V> {
      for (const [, value] of (this as any)[dataSymbol]) {
        yield value;
      }
    }

    forEach(
      callbackfn: (value: V, key: K, parent: this) => void,
      // tslint:disable-next-line:no-any
      thisArg?: any
    ): void {
      requiredArguments(
        `${this.constructor.name}.forEach`,
        arguments.length,
        1
      );
      callbackfn = callbackfn.bind(thisArg == null ? window : Object(thisArg));
      for (const [key, value] of (this as any)[dataSymbol]) {
        callbackfn(value, key, this);
      }
    }

    *[Symbol.iterator](): IterableIterator<[K, V]> {
      for (const entry of (this as any)[dataSymbol]) {
        yield entry;
      }
    }
  };

  // we want the Base class name to be the name of the class.
  Object.defineProperty(DomIterable, "name", {
    value: Base.name,
    configurable: true
  });

  return DomIterable;
}
// tslint:enable:no-any
