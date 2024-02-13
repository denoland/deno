// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { ascend } from "./comparators.ts";
import { BinarySearchTree } from "./binary_search_tree.ts";
import { Direction, RedBlackNode } from "./_red_black_node.ts";

/**
 * A red-black tree. This is a kind of self-balancing binary search tree. The
 * values are in ascending order by default, using JavaScript's built-in
 * comparison operators to sort the values.
 *
 * Red-Black Trees require fewer rotations than AVL Trees, so they can provide
 * faster insertions and removal operations. If you need faster lookups, you
 * should use an AVL Tree instead. AVL Trees are more strictly balanced than
 * Red-Black Trees, so they can provide faster lookups.
 *
 * | Method        | Average Case | Worst Case |
 * | ------------- | ------------ | ---------- |
 * | find(value)   | O(log n)     | O(log n)   |
 * | insert(value) | O(log n)     | O(log n)   |
 * | remove(value) | O(log n)     | O(log n)   |
 * | min()         | O(log n)     | O(log n)   |
 * | max()         | O(log n)     | O(log n)   |
 *
 * @example
 * ```ts
 * import {
 *   ascend,
 *   descend,
 *   RedBlackTree,
 * } from "https://deno.land/std@$STD_VERSION/data_structures/mod.ts";
 * import { assertEquals } from "https://deno.land/std@$STD_VERSION/assert/assert_equals.ts";
 *
 * const values = [3, 10, 13, 4, 6, 7, 1, 14];
 * const tree = new RedBlackTree<number>();
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
 * const invertedTree = new RedBlackTree<number>(descend);
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
 * const words = new RedBlackTree<string>((a, b) =>
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
export class RedBlackTree<T> extends BinarySearchTree<T> {
  declare protected root: RedBlackNode<T> | null;

  constructor(
    compare: (a: T, b: T) => number = ascend,
  ) {
    super(compare);
  }

  /** Creates a new red-black tree from an array like or iterable object. */
  static override from<T>(
    collection: ArrayLike<T> | Iterable<T> | RedBlackTree<T>,
  ): RedBlackTree<T>;
  static override from<T>(
    collection: ArrayLike<T> | Iterable<T> | RedBlackTree<T>,
    options: {
      Node?: typeof RedBlackNode;
      compare?: (a: T, b: T) => number;
    },
  ): RedBlackTree<T>;
  static override from<T, U, V>(
    collection: ArrayLike<T> | Iterable<T> | RedBlackTree<T>,
    options: {
      compare?: (a: U, b: U) => number;
      map: (value: T, index: number) => U;
      thisArg?: V;
    },
  ): RedBlackTree<U>;
  static override from<T, U, V>(
    collection: ArrayLike<T> | Iterable<T> | RedBlackTree<T>,
    options?: {
      compare?: (a: U, b: U) => number;
      map?: (value: T, index: number) => U;
      thisArg?: V;
    },
  ): RedBlackTree<U> {
    let result: RedBlackTree<U>;
    let unmappedValues: ArrayLike<T> | Iterable<T> = [];
    if (collection instanceof RedBlackTree) {
      result = new RedBlackTree(
        options?.compare ?? (collection as unknown as RedBlackTree<U>).compare,
      );
      if (options?.compare || options?.map) {
        unmappedValues = collection;
      } else {
        const nodes: RedBlackNode<U>[] = [];
        if (collection.root) {
          result.root = RedBlackNode.from(
            collection.root as unknown as RedBlackNode<U>,
          );
          nodes.push(result.root);
        }
        while (nodes.length) {
          const node: RedBlackNode<U> = nodes.pop()!;
          const left: RedBlackNode<U> | null = node.left
            ? RedBlackNode.from(node.left)
            : null;
          const right: RedBlackNode<U> | null = node.right
            ? RedBlackNode.from(node.right)
            : null;

          if (left) {
            left.parent = node;
            nodes.push(left);
          }
          if (right) {
            right.parent = node;
            nodes.push(right);
          }
        }
      }
    } else {
      result = (options?.compare
        ? new RedBlackTree(options.compare)
        : new RedBlackTree()) as RedBlackTree<U>;
      unmappedValues = collection;
    }
    const values: Iterable<U> = options?.map
      ? Array.from(unmappedValues, options.map, options.thisArg)
      : unmappedValues as U[];
    for (const value of values) result.insert(value);
    return result;
  }

  protected removeFixup(
    parent: RedBlackNode<T> | null,
    current: RedBlackNode<T> | null,
  ) {
    while (parent && !current?.red) {
      const direction: Direction = parent.left === current ? "left" : "right";
      const siblingDirection: Direction = direction === "right"
        ? "left"
        : "right";
      let sibling: RedBlackNode<T> | null = parent[siblingDirection];

      if (sibling?.red) {
        sibling.red = false;
        parent.red = true;
        this.rotateNode(parent, direction);
        sibling = parent[siblingDirection];
      }
      if (sibling) {
        if (!sibling.left?.red && !sibling.right?.red) {
          sibling!.red = true;
          current = parent;
          parent = current.parent;
        } else {
          if (!sibling[siblingDirection]?.red) {
            sibling[direction]!.red = false;
            sibling.red = true;
            this.rotateNode(sibling, siblingDirection);
            sibling = parent[siblingDirection!];
          }
          sibling!.red = parent.red;
          parent.red = false;
          sibling![siblingDirection]!.red = false;
          this.rotateNode(parent, direction);
          current = this.root;
          parent = null;
        }
      }
    }
    if (current) current.red = false;
  }

  /**
   * Adds the value to the binary search tree if it does not already exist in it.
   * Returns true if successful.
   */
  override insert(value: T): boolean {
    let node = this.insertNode(RedBlackNode, value) as (RedBlackNode<T> | null);
    if (node) {
      while (node.parent?.red) {
        let parent: RedBlackNode<T> = node.parent!;
        const parentDirection: Direction = parent.directionFromParent()!;
        const uncleDirection: Direction = parentDirection === "right"
          ? "left"
          : "right";
        const uncle: RedBlackNode<T> | null = parent.parent![uncleDirection] ??
          null;

        if (uncle?.red) {
          parent.red = false;
          uncle.red = false;
          parent.parent!.red = true;
          node = parent.parent!;
        } else {
          if (node === parent[uncleDirection]) {
            node = parent;
            this.rotateNode(node, parentDirection);
            parent = node.parent!;
          }
          parent.red = false;
          parent.parent!.red = true;
          this.rotateNode(parent.parent!, uncleDirection);
        }
      }
      this.root!.red = false;
    }
    return !!node;
  }

  /**
   * Removes node value from the binary search tree if found.
   * Returns true if found and removed.
   */
  override remove(value: T): boolean {
    const node = this.findNode(value) as (RedBlackNode<T> | null);

    if (!node) {
      return false;
    }

    const removedNode = this.removeNode(node) as (
      | RedBlackNode<T>
      | null
    );

    if (removedNode && !removedNode.red) {
      this.removeFixup(
        removedNode.parent,
        removedNode.left ?? removedNode.right,
      );
    }

    return true;
  }
}
