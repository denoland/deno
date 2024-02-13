// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { BinarySearchNode, Direction } from "./_binary_search_node.ts";
export type { Direction };

export class RedBlackNode<T> extends BinarySearchNode<T> {
  declare parent: RedBlackNode<T> | null;
  declare left: RedBlackNode<T> | null;
  declare right: RedBlackNode<T> | null;
  red: boolean;

  constructor(parent: RedBlackNode<T> | null, value: T) {
    super(parent, value);
    this.red = true;
  }

  static override from<T>(node: RedBlackNode<T>): RedBlackNode<T> {
    const copy: RedBlackNode<T> = new RedBlackNode(node.parent, node.value);
    copy.left = node.left;
    copy.right = node.right;
    copy.red = node.red;
    return copy;
  }
}
