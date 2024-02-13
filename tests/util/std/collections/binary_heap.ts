// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

export {
  /**
   * @deprecated (will be removed in 0.209.0) Import from {@link https://deno.land/std/data_structures/binary_heap.ts} instead.
   *
   * A priority queue implemented with a binary heap. The heap is in descending
   * order by default, using JavaScript's built-in comparison operators to sort
   * the values.
   *
   * | Method      | Average Case | Worst Case |
   * | ----------- | ------------ | ---------- |
   * | peek()      | O(1)         | O(1)       |
   * | pop()       | O(log n)     | O(log n)   |
   * | push(value) | O(1)         | O(log n)   |
   *
   * @example
   * ```ts
   * import {
   *   ascend,
   *   BinaryHeap,
   *   descend,
   * } from "https://deno.land/std@$STD_VERSION/data_structures/mod.ts";
   * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
   *
   * const maxHeap = new BinaryHeap<number>();
   * maxHeap.push(4, 1, 3, 5, 2);
   * assertEquals(maxHeap.peek(), 5);
   * assertEquals(maxHeap.pop(), 5);
   * assertEquals([...maxHeap], [4, 3, 2, 1]);
   * assertEquals([...maxHeap], []);
   *
   * const minHeap = new BinaryHeap<number>(ascend);
   * minHeap.push(4, 1, 3, 5, 2);
   * assertEquals(minHeap.peek(), 1);
   * assertEquals(minHeap.pop(), 1);
   * assertEquals([...minHeap], [2, 3, 4, 5]);
   * assertEquals([...minHeap], []);
   *
   * const words = new BinaryHeap<string>((a, b) => descend(a.length, b.length));
   * words.push("truck", "car", "helicopter", "tank");
   * assertEquals(words.peek(), "helicopter");
   * assertEquals(words.pop(), "helicopter");
   * assertEquals([...words], ["truck", "tank", "car"]);
   * assertEquals([...words], []);
   * ```
   */
  BinaryHeap,
} from "../data_structures/binary_heap.ts";

export {
  /**
   * @deprecated (will be removed in 0.209.0) Import from {@link https://deno.land/std/data_structures/comparators.ts} instead.
   *
   * Compares its two arguments for ascending order using JavaScript's built in comparison operators.
   */
  ascend,
  /**
   * @deprecated (will be removed in 0.209.0) Import from {@link https://deno.land/std/data_structures/comparators.ts} instead.
   *
   * Compares its two arguments for descending order using JavaScript's built in comparison operators.
   */
  descend,
} from "../data_structures/comparators.ts";
