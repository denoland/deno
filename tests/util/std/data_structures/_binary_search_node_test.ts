// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertStrictEquals } from "../assert/mod.ts";
import { BinarySearchNode } from "./_binary_search_node.ts";

let parent: BinarySearchNode<number>;
let child: BinarySearchNode<number>;
function beforeEach() {
  parent = new BinarySearchNode(null, 5);
  child = new BinarySearchNode(parent, 7);
  parent.right = child;
}

Deno.test("[collections/BinarySearchNode] constructor", () => {
  beforeEach();
  assertStrictEquals(parent.parent, null);
  assertStrictEquals(parent.left, null);
  assertStrictEquals(parent.right, child);
  assertStrictEquals(parent.value, 5);

  assertStrictEquals(child.parent, parent);
  assertStrictEquals(child.left, null);
  assertStrictEquals(child.right, null);
  assertStrictEquals(child.value, 7);
});

Deno.test("[collections/BinarySearchNode] from", () => {
  beforeEach();
  const parentClone: BinarySearchNode<number> = BinarySearchNode.from(parent);
  const childClone: BinarySearchNode<number> = BinarySearchNode.from(child);

  assertStrictEquals(parentClone.parent, null);
  assertStrictEquals(parentClone.left, null);
  assertStrictEquals(parentClone.right, child);
  assertStrictEquals(parentClone.value, 5);

  assertStrictEquals(childClone.parent, parent);
  assertStrictEquals(childClone.left, null);
  assertStrictEquals(childClone.right, null);
  assertStrictEquals(childClone.value, 7);
});

Deno.test("[collections/BinarySearchNode] directionFromParent", () => {
  beforeEach();
  const child2 = new BinarySearchNode(parent, 3);
  assertEquals(child2.directionFromParent(), null);
  parent.left = child2;
  assertEquals(child2.directionFromParent(), "left");
  assertEquals(parent.directionFromParent(), null);
  assertEquals(child.directionFromParent(), "right");
});

Deno.test("[collections/BinarySearchNode] findMinNode", () => {
  beforeEach();
  assertStrictEquals(parent.findMinNode(), parent);
  const child2 = new BinarySearchNode(parent, 3);
  parent.left = child2;
  assertStrictEquals(parent.findMinNode(), child2);
  const child3 = new BinarySearchNode(child2, 4);
  child2.right = child3;
  assertStrictEquals(parent.findMinNode(), child2);
  const child4 = new BinarySearchNode(child2, 2);
  child2.left = child4;
  assertStrictEquals(parent.findMinNode(), child4);
});

Deno.test("[collections/BinarySearchNode] findMaxNode", () => {
  beforeEach();
  assertStrictEquals(parent.findMaxNode(), child);
  const child2 = new BinarySearchNode(child, 6);
  child.left = child2;
  assertStrictEquals(parent.findMaxNode(), child);
  const child3 = new BinarySearchNode(child2, 6.5);
  child2.right = child3;
  assertStrictEquals(parent.findMaxNode(), child);
  const child4 = new BinarySearchNode(child2, 8);
  child.right = child4;
  assertStrictEquals(parent.findMaxNode(), child4);
  parent.right = null;
  assertStrictEquals(parent.findMaxNode(), parent);
});

Deno.test("[collections/BinarySearchNode] findSuccessorNode", () => {
  beforeEach();
  assertStrictEquals(parent.findSuccessorNode(), child);
  assertStrictEquals(child.findSuccessorNode(), null);
  const child2 = new BinarySearchNode(child, 6);
  child.left = child2;
  assertStrictEquals(parent.findSuccessorNode(), child2);
  assertStrictEquals(child.findSuccessorNode(), null);
});
