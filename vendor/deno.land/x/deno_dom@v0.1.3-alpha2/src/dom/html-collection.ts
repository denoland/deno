import type { Element } from "./element.ts";

const HTMLCollectionFakeClass: any = (() => {
  return class HTMLCollection {
    constructor() {
      throw new TypeError("Illegal constructor");
    }

    static [Symbol.hasInstance](value: any) {
      return value.constructor === HTMLCollectionClass;
    }
  }
})();

export const HTMLCollectionMutatorSym = Symbol();

// We define the `HTMLCollection` inside a closure to ensure that its
// `.name === "HTMLCollection"` property stays intact, as we need to manipulate
// its prototype and completely change its TypeScript-recognized type.
const HTMLCollectionClass: any = (() => {
  // @ts-ignore
  class HTMLCollection extends Array<Element> {
    // @ts-ignore
    forEach(
      cb: (node: Element, index: number, nodeList: Element[]) => void, 
      thisArg: HTMLCollection | undefined = undefined
    ) {
      super.forEach(cb, thisArg);
    }

    item(index: number): Element | null {
      return this[index] ?? null;
    }

    [HTMLCollectionMutatorSym]() {
      return {
        push: Array.prototype.push.bind(this),

        splice: Array.prototype.splice.bind(this),

        indexOf: Array.prototype.indexOf.bind(this),
      }
    }
  }

  return HTMLCollection;
})();

for (const staticMethod of [
  "from",
  "isArray",
  "of",
]) {
  HTMLCollectionClass[staticMethod] = undefined;
}

for (const instanceMethod of [
  "concat",
  "copyWithin",
  "every",
  "fill",
  "filter",
  "find",
  "findIndex",
  "flat",
  "flatMap",
  "includes",
  "indexOf",
  "join",
  "lastIndexOf",
  "map",
  "pop",
  "push",
  "reduce",
  "reduceRight",
  "reverse",
  "shift",
  "slice",
  "some",
  "sort",
  "splice",
  "toLocaleString",
  "unshift",

  // Unlike NodeList, HTMLCollection also doesn't implement these
  "entries",
  "forEach",
  "keys",
  "values",
]) {
  HTMLCollectionClass.prototype[instanceMethod] = undefined;
}

export interface HTMLCollection {
  new(): HTMLCollection;
  readonly [index: number]: Element;
  readonly length: number;
  [Symbol.iterator](): Generator<Element>;

  item(index: number): Element;
  [HTMLCollectionMutatorSym](): HTMLCollectionMutator;
}

export interface HTMLCollectionPublic extends HTMLCollection {
  [HTMLCollectionMutatorSym]: never;
}

export interface HTMLCollectionMutator {
  push(...elements: Element[]): number;
  splice(start: number, deleteCount?: number, ...items: Element[]): Element[];
  indexOf(element: Element, fromIndex?: number | undefined): number;
}

export const HTMLCollection = <HTMLCollection> HTMLCollectionClass;
export const HTMLCollectionPublic = <HTMLCollectionPublic> HTMLCollectionFakeClass;

