// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

export {
  /**
   * @deprecated (will be removed in 0.209.0) Import from {@link https://deno.land/std/data_structures/binary_search_tree.ts} instead.
   *
   * An unbalanced binary search tree. The values are in ascending order by default,
   * using JavaScript's built-in comparison operators to sort the values.
   *
   * For performance, it's recommended that you use a self-balancing binary search
   * tree instead of this one unless you are extending this to create a
   * self-balancing tree. See RedBlackTree for an example of how BinarySearchTree
   *  can be extended to create a self-balancing binary search tree.
   *
   * | Method        | Average Case | Worst Case |
   * | ------------- | ------------ | ---------- |
   * | find(value)   | O(log n)     | O(n)       |
   * | insert(value) | O(log n)     | O(n)       |
   * | remove(value) | O(log n)     | O(n)       |
   * | min()         | O(log n)     | O(n)       |
   * | max()         | O(log n)     | O(n)       |
   *
   * @example
   * ```ts
   * import {
   *   ascend,
   *   descend,
   *   BinarySearchTree,
   * } from "https://deno.land/std@$STD_VERSION/data_structures/mod.ts";
   * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
   *
   * const values = [3, 10, 13, 4, 6, 7, 1, 14];
   * const tree = new BinarySearchTree<number>();
   * values.forEach((value) => tree.insert(value));
   * assertEquals([...tree], [1, 3, 4, 6, 7, 10, 13, 14]);
   * assertEquals(tree.min(), 1);
   * assertEquals(tree.max(), 14);
   * assertEquals(tree.find(42), null);
   * assertEquals(tree.find(7), 7);
   * assertEquals(tree.remove(42), false);
   * assertEquals(tree.remove(7), true);
   * assertEquals([...tree], [1, 3, 4, 6, 10, 13, 14]);
   *
   * const invertedTree = new BinarySearchTree<number>(descend);
   * values.forEach((value) => invertedTree.insert(value));
   * assertEquals([...invertedTree], [14, 13, 10, 7, 6, 4, 3, 1]);
   * assertEquals(invertedTree.min(), 14);
   * assertEquals(invertedTree.max(), 1);
   * assertEquals(invertedTree.find(42), null);
   * assertEquals(invertedTree.find(7), 7);
   * assertEquals(invertedTree.remove(42), false);
   * assertEquals(invertedTree.remove(7), true);
   * assertEquals([...invertedTree], [14, 13, 10, 6, 4, 3, 1]);
   *
   * const words = new BinarySearchTree<string>((a, b) =>
   *   ascend(a.length, b.length) || ascend(a, b)
   * );
   * ["truck", "car", "helicopter", "tank", "train", "suv", "semi", "van"]
   *   .forEach((value) => words.insert(value));
   * assertEquals([...words], [
   *   "car",
   *   "suv",
   *   "van",
   *   "semi",
   *   "tank",
   *   "train",
   *   "truck",
   *   "helicopter",
   * ]);
   * assertEquals(words.min(), "car");
   * assertEquals(words.max(), "helicopter");
   * assertEquals(words.find("scooter"), null);
   * assertEquals(words.find("tank"), "tank");
   * assertEquals(words.remove("scooter"), false);
   * assertEquals(words.remove("tank"), true);
   * assertEquals([...words], [
   *   "car",
   *   "suv",
   *   "van",
   *   "semi",
   *   "train",
   *   "truck",
   *   "helicopter",
   * ]);
   * ```
   */
  BinarySearchTree,
} from "../data_structures/binary_search_tree.ts";
