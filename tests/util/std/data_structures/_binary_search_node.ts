// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

export type Direction = "left" | "right";

export class BinarySearchNode<T> {
  left: BinarySearchNode<T> | null;
  right: BinarySearchNode<T> | null;
  constructor(public parent: BinarySearchNode<T> | null, public value: T) {
    this.left = null;
    this.right = null;
  }

  static from<T>(node: BinarySearchNode<T>): BinarySearchNode<T> {
    const copy: BinarySearchNode<T> = new BinarySearchNode(
      node.parent,
      node.value,
    );
    copy.left = node.left;
    copy.right = node.right;
    return copy;
  }

  directionFromParent(): Direction | null {
    return this.parent === null
      ? null
      : this === this.parent.left
      ? "left"
      : this === this.parent.right
      ? "right"
      : null;
  }

  findMinNode(): BinarySearchNode<T> {
    let minNode: BinarySearchNode<T> | null = this.left;
    while (minNode?.left) minNode = minNode.left;
    return minNode ?? this;
  }

  findMaxNode(): BinarySearchNode<T> {
    let maxNode: BinarySearchNode<T> | null = this.right;
    while (maxNode?.right) maxNode = maxNode.right;
    return maxNode ?? this;
  }

  findSuccessorNode(): BinarySearchNode<T> | null {
    if (this.right !== null) return this.right.findMinNode();
    let parent: BinarySearchNode<T> | null = this.parent;
    let direction: Direction | null = this.directionFromParent();
    while (parent && direction === "right") {
      direction = parent.directionFromParent();
      parent = parent.parent;
    }
    return parent;
  }
}
