// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertStrictEquals } from "../assert/mod.ts";
import { RedBlackNode } from "./_red_black_node.ts";

Deno.test("[collections/RedBlackNode] constructor and from", () => {
  const parent: RedBlackNode<number> = new RedBlackNode(null, 5);
  const child: RedBlackNode<number> = new RedBlackNode(parent, 7);
  parent.left = child;
  assertStrictEquals(parent.red, true);
  parent.red = false;
  const parentClone: RedBlackNode<number> = RedBlackNode.from(parent);
  const childClone: RedBlackNode<number> = RedBlackNode.from(child);

  assertStrictEquals(parent.parent, null);
  assertStrictEquals(parent.left, child);
  assertStrictEquals(parent.right, null);
  assertStrictEquals(parent.value, 5);
  assertStrictEquals(parent.red, false);

  assertStrictEquals(child.parent, parent);
  assertStrictEquals(child.left, null);
  assertStrictEquals(child.right, null);
  assertStrictEquals(child.value, 7);
  assertStrictEquals(child.red, true);

  assertStrictEquals(parentClone.parent, null);
  assertStrictEquals(parentClone.left, child);
  assertStrictEquals(parentClone.right, null);
  assertStrictEquals(parentClone.value, 5);
  assertStrictEquals(parentClone.red, false);

  assertStrictEquals(childClone.parent, parent);
  assertStrictEquals(childClone.left, null);
  assertStrictEquals(childClone.right, null);
  assertStrictEquals(childClone.value, 7);
  assertStrictEquals(childClone.red, true);
});
