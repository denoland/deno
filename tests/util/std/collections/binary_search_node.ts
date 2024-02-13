// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

export {
  /**
   * @deprecated (will be removed in 0.209.0) Use {@linkcode BinarySearchTree} from {@link https://deno.land/std/data_structures/binary_search_tree.ts} instead.
   */
  BinarySearchNode,
} from "../data_structures/_binary_search_node.ts";

/** @deprecated (will be removed in 0.209.0) Use `"left" | "right"` union class instead */
export type Direction = "left" | "right";
