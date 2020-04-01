// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Utility functions for DOM nodes
import * as domTypes from "./dom_types.ts";

export function getDOMStringList(arr: string[]): domTypes.DOMStringList {
  Object.defineProperties(arr, {
    contains: {
      value(searchElement: string): boolean {
        return arr.includes(searchElement);
      },
      enumerable: true,
    },
    item: {
      value(idx: number): string | null {
        return idx in arr ? arr[idx] : null;
      },
    },
  });
  return (arr as unknown) as domTypes.DOMStringList;
}

export function isNode(nodeImpl: domTypes.EventTarget | null): boolean {
  return Boolean(nodeImpl && "nodeType" in nodeImpl);
}

export function isShadowRoot(nodeImpl: domTypes.EventTarget | null): boolean {
  return Boolean(
    nodeImpl &&
      nodeImpl[domTypes.eventTargetNodeType] ===
        domTypes.NodeType.DOCUMENT_FRAGMENT_NODE &&
      nodeImpl[domTypes.eventTargetHost] != null
  );
}

export function isSlotable(nodeImpl: domTypes.EventTarget | null): boolean {
  return Boolean(
    nodeImpl &&
      (nodeImpl[domTypes.eventTargetNodeType] ===
        domTypes.NodeType.ELEMENT_NODE ||
        nodeImpl[domTypes.eventTargetNodeType] === domTypes.NodeType.TEXT_NODE)
  );
}

// https://dom.spec.whatwg.org/#node-trees
// const domSymbolTree = Symbol("DOM Symbol Tree");

// https://dom.spec.whatwg.org/#concept-shadow-including-inclusive-ancestor
export function isShadowInclusiveAncestor(
  ancestor: domTypes.EventTarget | null,
  node: domTypes.EventTarget | null
): boolean {
  while (isNode(node)) {
    if (node === ancestor) {
      return true;
    }

    if (isShadowRoot(node)) {
      node = node && node[domTypes.eventTargetHost];
    } else {
      node = null; // domSymbolTree.parent(node);
    }
  }

  return false;
}

export function getRoot(
  node: domTypes.EventTarget | null
): domTypes.EventTarget | null {
  const root = node;

  // for (const ancestor of domSymbolTree.ancestorsIterator(node)) {
  //   root = ancestor;
  // }

  return root;
}

// https://dom.spec.whatwg.org/#retarget
export function retarget(
  a: domTypes.EventTarget | null,
  b: domTypes.EventTarget
): domTypes.EventTarget | null {
  while (true) {
    if (!isNode(a)) {
      return a;
    }

    const aRoot = getRoot(a);

    if (aRoot) {
      if (
        !isShadowRoot(aRoot) ||
        (isNode(b) && isShadowInclusiveAncestor(aRoot, b))
      ) {
        return a;
      }

      a = aRoot[domTypes.eventTargetHost];
    }
  }
}
