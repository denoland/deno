import type { Node } from "./node.ts";

const NodeListFakeClass: any = (() => {
  return class NodeList {
    constructor() {
      throw new TypeError("Illegal constructor");
    }

    static [Symbol.hasInstance](value: any) {
      return value.constructor === NodeListClass;
    }
  }
})();

export const nodeListMutatorSym = Symbol();

// We define the `NodeList` inside a closure to ensure that its
// `.name === "NodeList"` property stays intact, as we need to manipulate
// its prototype and completely change its TypeScript-recognized type.
const NodeListClass: any = (() => {
  // @ts-ignore
  class NodeList extends Array<Node> {
    // @ts-ignore
    forEach(
      cb: (node: Node, index: number, nodeList: Node[]) => void, 
      thisArg: NodeList | undefined = undefined
    ) {
      super.forEach(cb, thisArg);
    }

    item(index: number): Node | null {
      return this[index] ?? null;
    }

    [nodeListMutatorSym]() {
      return {
        push: Array.prototype.push.bind(this),

        splice: Array.prototype.splice.bind(this),

        indexOf: Array.prototype.indexOf.bind(this),
      }
    }
  }

  return NodeList;
})();

for (const staticMethod of [
  "from",
  "isArray",
  "of",
]) {
  NodeListClass[staticMethod] = undefined;
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
]) {
  NodeListClass.prototype[instanceMethod] = undefined;
}

export interface NodeList {
  new(): NodeList;
  readonly [index: number]: Node;
  readonly length: number;
  [Symbol.iterator](): Generator<Node>;

  item(index: number): Node;
  [nodeListMutatorSym](): NodeListMutator;
}

export interface NodeListPublic extends NodeList {
  [nodeListMutatorSym]: never;
}

export interface NodeListMutator {
  push(...nodes: Node[]): number;
  splice(start: number, deleteCount?: number, ...items: Node[]): Node[];
  indexOf(node: Node, fromIndex?: number | undefined): number;
}

export const NodeList = <NodeList> NodeListClass;
export const NodeListPublic = <NodeListPublic> NodeListFakeClass;

