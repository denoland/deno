// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertStrictEquals } from "../assert/mod.ts";
import { RedBlackTree } from "./red_black_tree.ts";
import { ascend, descend } from "./comparators.ts";
import { Container, MyMath } from "./_test_utils.ts";

Deno.test("[collections/RedBlackTree] with default ascend comparator", () => {
  const trees: RedBlackTree<number>[] = [
    new RedBlackTree(),
    new RedBlackTree(),
  ];
  const values: number[] = [-10, 9, -1, 100, 1, 0, -100, 10, -9];

  const expectedMin: number[][] = [
    [-10, -10, -10, -10, -10, -10, -100, -100, -100],
    [-9, -9, -100, -100, -100, -100, -100, -100, -100],
  ];
  const expectedMax: number[][] = [
    [-10, 9, 9, 100, 100, 100, 100, 100, 100],
    [-9, 10, 10, 10, 10, 100, 100, 100, 100],
  ];
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, 0);
    assertEquals(trees[i].isEmpty(), true);
    for (let j = 0; j < values.length; j++) {
      assertEquals(trees[i].find(values[j]), null);
      assertEquals(trees[i].insert(values[j]), true);
      assertEquals(trees[i].find(values[j]), values[j]);
      assertEquals(trees[i].size, j + 1);
      assertEquals(trees[i].isEmpty(), false);
      assertEquals(trees[i].min(), expectedMin[i][j]);
      assertEquals(trees[i].max(), expectedMax[i][j]);
    }
    for (let j = 0; j < values.length; j++) {
      assertEquals(trees[i].insert(values[j]), false);
      assertEquals(trees[i].size, values.length);
      assertEquals(trees[i].isEmpty(), false);
      assertEquals(trees[i].min(), -100);
      assertEquals(trees[i].max(), 100);
    }
    values.reverse();
  }

  for (let i = 0; i < 2; i++) {
    assertEquals(
      [...trees[i].lnrValues()],
      [-100, -10, -9, -1, 0, 1, 9, 10, 100],
    );
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);

    assertEquals(
      [...trees[i]],
      [-100, -10, -9, -1, 0, 1, 9, 10, 100],
    );
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);

    assertEquals(
      [...trees[i].rnlValues()],
      [100, 10, 9, 1, 0, -1, -9, -10, -100],
    );
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].nlrValues()],
    [-1, -10, -100, -9, 9, 1, 0, 100, 10],
  );
  assertEquals(
    [...trees[1].nlrValues()],
    [-9, -100, -10, 1, 0, -1, 10, 9, 100],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].lrnValues()],
    [-100, -9, -10, 0, 1, 10, 100, 9, -1],
  );
  assertEquals(
    [...trees[1].lrnValues()],
    [-10, -100, -1, 0, 9, 100, 10, 1, -9],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].lvlValues()],
    [-1, -10, 9, -100, -9, 1, 100, 0, 10],
  );
  assertEquals(
    [...trees[1].lvlValues()],
    [-9, -100, 1, -10, 0, 10, -1, 9, 100],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  for (let i = 0; i < 2; i++) {
    const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 10, 100];
    for (let j = 0; j < values.length; j++) {
      assertEquals(trees[i].size, values.length - j);
      assertEquals(trees[i].isEmpty(), false);
      assertEquals(trees[i].find(values[j]), values[j]);

      assertEquals(trees[i].remove(values[j]), true);
      expected.splice(expected.indexOf(values[j]), 1);
      assertEquals([...trees[i]], expected);
      assertEquals(trees[i].find(values[j]), null);

      assertEquals(trees[i].remove(values[j]), false);
      assertEquals([...trees[i]], expected);
      assertEquals(trees[i].find(values[j]), null);
    }
    assertEquals(trees[i].size, 0);
    assertEquals(trees[i].isEmpty(), true);
  }
});

Deno.test("[collections/RedBlackTree] with descend comparator", () => {
  const trees: RedBlackTree<number>[] = [
    new RedBlackTree(descend),
    new RedBlackTree(descend),
  ];
  const values: number[] = [-10, 9, -1, 100, 1, 0, -100, 10, -9];

  const expectedMin: number[][] = [
    [-10, 9, 9, 100, 100, 100, 100, 100, 100],
    [-9, 10, 10, 10, 10, 100, 100, 100, 100, 100],
  ];
  const expectedMax: number[][] = [
    [-10, -10, -10, -10, -10, -10, -100, -100, -100],
    [-9, -9, -100, -100, -100, -100, -100, -100, -100],
  ];
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, 0);
    assertEquals(trees[i].isEmpty(), true);
    for (let j = 0; j < values.length; j++) {
      assertEquals(trees[i].find(values[j]), null);
      assertEquals(trees[i].insert(values[j]), true);
      assertEquals(trees[i].find(values[j]), values[j]);
      assertEquals(trees[i].size, j + 1);
      assertEquals(trees[i].isEmpty(), false);
      assertEquals(trees[i].min(), expectedMin[i][j]);
      assertEquals(trees[i].max(), expectedMax[i][j]);
    }
    for (let j = 0; j < values.length; j++) {
      assertEquals(trees[i].insert(values[j]), false);
      assertEquals(trees[i].size, values.length);
      assertEquals(trees[i].isEmpty(), false);
      assertEquals(trees[i].min(), 100);
      assertEquals(trees[i].max(), -100);
    }
    values.reverse();
  }

  for (let i = 0; i < 2; i++) {
    assertEquals(
      [...trees[i].lnrValues()],
      [100, 10, 9, 1, 0, -1, -9, -10, -100],
    );
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);

    assertEquals(
      [...trees[i]],
      [100, 10, 9, 1, 0, -1, -9, -10, -100],
    );
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);

    assertEquals(
      [...trees[i].rnlValues()],
      [-100, -10, -9, -1, 0, 1, 9, 10, 100],
    );
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].nlrValues()],
    [-1, 9, 100, 10, 1, 0, -10, -9, -100],
  );
  assertEquals(
    [...trees[1].nlrValues()],
    [-9, 1, 10, 100, 9, 0, -1, -100, -10],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].lrnValues()],
    [10, 100, 0, 1, 9, -9, -100, -10, -1],
  );
  assertEquals(
    [...trees[1].lrnValues()],
    [100, 9, 10, -1, 0, 1, -10, -100, -9],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].lvlValues()],
    [-1, 9, -10, 100, 1, -9, -100, 10, 0],
  );
  assertEquals(
    [...trees[1].lvlValues()],
    [-9, 1, -100, 10, 0, -10, 100, 9, -1],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  for (let i = 0; i < 2; i++) {
    const expected: number[] = [100, 10, 9, 1, 0, -1, -9, -10, -100];
    for (let j = 0; j < values.length; j++) {
      assertEquals(trees[i].size, values.length - j);
      assertEquals(trees[i].isEmpty(), false);
      assertEquals(trees[i].find(values[j]), values[j]);

      assertEquals(trees[i].remove(values[j]), true);
      expected.splice(expected.indexOf(values[j]), 1);
      assertEquals([...trees[i]], expected);
      assertEquals(trees[i].find(values[j]), null);

      assertEquals(trees[i].remove(values[j]), false);
      assertEquals([...trees[i]], expected);
      assertEquals(trees[i].find(values[j]), null);
    }
    assertEquals(trees[i].size, 0);
    assertEquals(trees[i].isEmpty(), true);
  }
});

Deno.test("[collections/RedBlackTree] containing objects", () => {
  const tree: RedBlackTree<Container> = new RedBlackTree((
    a: Container,
    b: Container,
  ) => ascend(a.id, b.id));
  const ids: number[] = [-10, 9, -1, 100, 1, 0, -100, 10, -9];

  for (let i = 0; i < ids.length; i++) {
    const newContainer: Container = { id: ids[i], values: [] };
    assertEquals(tree.find(newContainer), null);
    assertEquals(tree.insert(newContainer), true);
    newContainer.values.push(i - 1, i, i + 1);
    assertStrictEquals(tree.find({ id: ids[i], values: [] }), newContainer);
    assertEquals(tree.size, i + 1);
    assertEquals(tree.isEmpty(), false);
  }
  for (let i = 0; i < ids.length; i++) {
    const newContainer: Container = { id: ids[i], values: [] };
    assertEquals(
      tree.find({ id: ids[i] } as Container),
      { id: ids[i], values: [i - 1, i, i + 1] },
    );
    assertEquals(tree.insert(newContainer), false);
    assertEquals(
      tree.find({ id: ids[i], values: [] }),
      { id: ids[i], values: [i - 1, i, i + 1] },
    );
    assertEquals(tree.size, ids.length);
    assertEquals(tree.isEmpty(), false);
  }

  assertEquals(
    [...tree].map((container) => container.id),
    [-100, -10, -9, -1, 0, 1, 9, 10, 100],
  );
  assertEquals(tree.size, ids.length);
  assertEquals(tree.isEmpty(), false);

  const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 10, 100];
  for (let i = 0; i < ids.length; i++) {
    assertEquals(tree.size, ids.length - i);
    assertEquals(tree.isEmpty(), false);
    assertEquals(
      tree.find({ id: ids[i], values: [] }),
      { id: ids[i], values: [i - 1, i, i + 1] },
    );

    assertEquals(tree.remove({ id: ids[i], values: [] }), true);
    expected.splice(expected.indexOf(ids[i]), 1);
    assertEquals([...tree].map((container) => container.id), expected);
    assertEquals(tree.find({ id: ids[i], values: [] }), null);

    assertEquals(tree.remove({ id: ids[i], values: [] }), false);
    assertEquals([...tree].map((container) => container.id), expected);
    assertEquals(tree.find({ id: ids[i], values: [] }), null);
  }
  assertEquals(tree.size, 0);
  assertEquals(tree.isEmpty(), true);
});

Deno.test("[collections/RedBlackTree] from Iterable", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const originalValues: number[] = Array.from(values);
  const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 10, 100];
  let tree: RedBlackTree<number> = RedBlackTree.from(values);
  assertEquals(values, originalValues);
  assertEquals([...tree], expected);
  assertEquals([...tree.nlrValues()], [-1, -10, -100, -9, 9, 1, 0, 100, 10]);
  assertEquals([...tree.lvlValues()], [-1, -10, 9, -100, -9, 1, 100, 0, 10]);

  tree = RedBlackTree.from(values, { compare: descend });
  assertEquals(values, originalValues);
  assertEquals([...tree].reverse(), expected);
  assertEquals([...tree.nlrValues()], [-1, 9, 100, 10, 1, 0, -10, -9, -100]);
  assertEquals([...tree.lvlValues()], [-1, 9, -10, 100, 1, -9, -100, 10, 0]);

  tree = RedBlackTree.from(values, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...tree], expected.map((v: number) => 2 * v));
  assertEquals([...tree.nlrValues()], [-2, -20, -200, -18, 18, 2, 0, 200, 20]);
  assertEquals([...tree.lvlValues()], [-2, -20, 18, -200, -18, 2, 200, 0, 20]);

  const math = new MyMath();
  tree = RedBlackTree.from(values, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals(values, originalValues);
  assertEquals([...tree], expected.map((v: number) => 3 * v));
  assertEquals([...tree.nlrValues()], [-3, -30, -300, -27, 27, 3, 0, 300, 30]);
  assertEquals([...tree.lvlValues()], [-3, -30, 27, -300, -27, 3, 300, 0, 30]);

  tree = RedBlackTree.from(values, {
    compare: descend,
    map: (v: number) => 2 * v,
  });
  assertEquals(values, originalValues);
  assertEquals([...tree].reverse(), expected.map((v: number) => 2 * v));
  assertEquals([...tree.nlrValues()], [-2, 18, 200, 20, 2, 0, -20, -18, -200]);
  assertEquals([...tree.lvlValues()], [-2, 18, -20, 200, 2, -18, -200, 20, 0]);

  tree = RedBlackTree.from(values, {
    compare: descend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals(values, originalValues);
  assertEquals([...tree].reverse(), expected.map((v: number) => 3 * v));
  assertEquals([...tree.nlrValues()], [-3, 27, 300, 30, 3, 0, -30, -27, -300]);
  assertEquals([...tree.lvlValues()], [-3, 27, -30, 300, 3, -27, -300, 30, 0]);
});

Deno.test("[collections/RedBlackTree] from RedBlackTree with default ascend comparator", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 10, 100];
  const originalTree: RedBlackTree<number> = new RedBlackTree();
  for (const value of values) originalTree.insert(value);
  let tree: RedBlackTree<number> = RedBlackTree.from(originalTree);
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected);
  assertEquals([...tree.nlrValues()], [...originalTree.nlrValues()]);
  assertEquals([...tree.lvlValues()], [...originalTree.lvlValues()]);

  tree = RedBlackTree.from(originalTree, { compare: descend });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected);
  assertEquals([...tree.nlrValues()], [-1, 1, 10, 100, 9, 0, -10, -9, -100]);
  assertEquals([...tree.lvlValues()], [-1, 1, -10, 10, 0, -9, -100, 100, 9]);

  tree = RedBlackTree.from(originalTree, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected.map((v: number) => 2 * v));
  assertEquals([...tree.nlrValues()], [-2, -20, -200, -18, 2, 0, 20, 18, 200]);
  assertEquals([...tree.lvlValues()], [-2, -20, 2, -200, -18, 0, 20, 18, 200]);

  const math = new MyMath();
  tree = RedBlackTree.from(originalTree, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected.map((v: number) => 3 * v));
  assertEquals([...tree.nlrValues()], [-3, -30, -300, -27, 3, 0, 30, 27, 300]);
  assertEquals([...tree.lvlValues()], [-3, -30, 3, -300, -27, 0, 30, 27, 300]);

  tree = RedBlackTree.from(originalTree, {
    compare: descend,
    map: (v: number) => 2 * v,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected.map((v: number) => 2 * v));
  assertEquals([...tree.nlrValues()], [-2, 2, 20, 200, 18, 0, -20, -18, -200]);
  assertEquals([...tree.lvlValues()], [-2, 2, -20, 20, 0, -18, -200, 200, 18]);

  tree = RedBlackTree.from(originalTree, {
    compare: descend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected.map((v: number) => 3 * v));
  assertEquals([...tree.nlrValues()], [-3, 3, 30, 300, 27, 0, -30, -27, -300]);
  assertEquals([...tree.lvlValues()], [-3, 3, -30, 30, 0, -27, -300, 300, 27]);
});

Deno.test("[collections/RedBlackTree] from RedBlackTree with descend comparator", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const expected: number[] = [100, 10, 9, 1, 0, -1, -9, -10, -100];
  const originalTree: RedBlackTree<number> = new RedBlackTree(descend);
  for (const value of values) originalTree.insert(value);
  let tree: RedBlackTree<number> = RedBlackTree.from(originalTree);
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected);
  assertEquals([...tree.nlrValues()], [...originalTree.nlrValues()]);
  assertEquals([...tree.lvlValues()], [...originalTree.lvlValues()]);

  tree = RedBlackTree.from(originalTree, { compare: ascend });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected);
  assertEquals([...tree.nlrValues()], [1, -1, -10, -100, -9, 0, 10, 9, 100]);
  assertEquals([...tree.lvlValues()], [1, -1, 10, -10, 0, 9, 100, -100, -9]);

  tree = RedBlackTree.from(originalTree, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected.map((v: number) => 2 * v));
  assertEquals([...tree.nlrValues()], [2, 20, 200, 18, -2, 0, -20, -18, -200]);
  assertEquals([...tree.lvlValues()], [2, 20, -2, 200, 18, 0, -20, -18, -200]);

  const math = new MyMath();
  tree = RedBlackTree.from(originalTree, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected.map((v: number) => 3 * v));
  assertEquals([...tree.nlrValues()], [3, 30, 300, 27, -3, 0, -30, -27, -300]);
  assertEquals([...tree.lvlValues()], [3, 30, -3, 300, 27, 0, -30, -27, -300]);

  tree = RedBlackTree.from(originalTree, {
    compare: ascend,
    map: (v: number) => 2 * v,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected.map((v: number) => 2 * v));
  assertEquals([...tree.nlrValues()], [2, -2, -20, -200, -18, 0, 20, 18, 200]);
  assertEquals([...tree.lvlValues()], [2, -2, 20, -20, 0, 18, 200, -200, -18]);

  tree = RedBlackTree.from(originalTree, {
    compare: ascend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected.map((v: number) => 3 * v));
  assertEquals([...tree.nlrValues()], [3, -3, -30, -300, -27, 0, 30, 27, 300]);
  assertEquals([...tree.lvlValues()], [3, -3, 30, -30, 0, 27, 300, -300, -27]);
});

Deno.test("[collections/RedBlackTree] insert rebalance left", () => {
  let values: number[] = [8, 4, 10, 0, 6, 11, -2, 2];
  let tree: RedBlackTree<number> = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [8, 4, 0, -2, 2, 6, 10, 11]);
  assertEquals([...tree.lvlValues()], [8, 4, 10, 0, 6, 11, -2, 2]);
  assertEquals(tree.insert(-3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, 2, 8, 6, 10, 11]);
  assertEquals([...tree.lvlValues()], [4, 0, 8, -2, 2, 6, 10, -3, 11]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(-1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -1, 2, 8, 6, 10, 11]);
  assertEquals([...tree.lvlValues()], [4, 0, 8, -2, 2, 6, 10, -1, 11]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 1, 8, 6, 10, 11]);
  assertEquals([...tree.lvlValues()], [4, 0, 8, -2, 2, 6, 10, 1, 11]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 3, 8, 6, 10, 11]);
  assertEquals([...tree.lvlValues()], [4, 0, 8, -2, 2, 6, 10, 3, 11]);

  values = [4, -4, 6, -5, 0, 7, -2, 2];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, -4, -5, 0, -2, 2, 6, 7]);
  assertEquals([...tree.lvlValues()], [4, -4, 6, -5, 0, 7, -2, 2]);
  assertEquals(tree.insert(-3), true);
  assertEquals([...tree.nlrValues()], [0, -4, -5, -2, -3, 4, 2, 6, 7]);
  assertEquals([...tree.lvlValues()], [0, -4, 4, -5, -2, 2, 6, -3, 7]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(-1), true);
  assertEquals([...tree.nlrValues()], [0, -4, -5, -2, -1, 4, 2, 6, 7]);
  assertEquals([...tree.lvlValues()], [0, -4, 4, -5, -2, 2, 6, -1, 7]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(1), true);
  assertEquals([...tree.nlrValues()], [0, -4, -5, -2, 4, 2, 1, 6, 7]);
  assertEquals([...tree.lvlValues()], [0, -4, 4, -5, -2, 2, 6, 1, 7]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(3), true);
  assertEquals([...tree.nlrValues()], [0, -4, -5, -2, 4, 2, 3, 6, 7]);
  assertEquals([...tree.lvlValues()], [0, -4, 4, -5, -2, 2, 6, 3, 7]);
});

Deno.test("[collections/RedBlackTree] insert rebalance right", () => {
  let values: number[] = [-4, -6, 4, 0, 6, -7, -2, 2];
  let tree: RedBlackTree<number> = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -6, -7, 4, 0, -2, 2, 6]);
  assertEquals([...tree.lvlValues()], [-4, -6, 4, -7, 0, 6, -2, 2]);
  assertEquals(tree.insert(-3), true);
  assertEquals([...tree.nlrValues()], [0, -4, -6, -7, -2, -3, 4, 2, 6]);
  assertEquals([...tree.lvlValues()], [0, -4, 4, -6, -2, 2, 6, -7, -3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(-1), true);
  assertEquals([...tree.nlrValues()], [0, -4, -6, -7, -2, -1, 4, 2, 6]);
  assertEquals([...tree.lvlValues()], [0, -4, 4, -6, -2, 2, 6, -7, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(1), true);
  assertEquals([...tree.nlrValues()], [0, -4, -6, -7, -2, 4, 2, 1, 6]);
  assertEquals([...tree.lvlValues()], [0, -4, 4, -6, -2, 2, 6, -7, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(3), true);
  assertEquals([...tree.nlrValues()], [0, -4, -6, -7, -2, 4, 2, 3, 6]);
  assertEquals([...tree.lvlValues()], [0, -4, 4, -6, -2, 2, 6, -7, 3]);

  values = [-8, -10, -4, -11, -6, 0, -2, 2];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-8, -10, -11, -4, -6, 0, -2, 2]);
  assertEquals([...tree.lvlValues()], [-8, -10, -4, -11, -6, 0, -2, 2]);
  assertEquals(tree.insert(-3), true);
  assertEquals([...tree.nlrValues()], [-4, -8, -10, -11, -6, 0, -2, -3, 2]);
  assertEquals([...tree.lvlValues()], [-4, -8, 0, -10, -6, -2, 2, -11, -3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(-1), true);
  assertEquals([...tree.nlrValues()], [-4, -8, -10, -11, -6, 0, -2, -1, 2]);
  assertEquals([...tree.lvlValues()], [-4, -8, 0, -10, -6, -2, 2, -11, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(1), true);
  assertEquals([...tree.nlrValues()], [-4, -8, -10, -11, -6, 0, -2, 2, 1]);
  assertEquals([...tree.lvlValues()], [-4, -8, 0, -10, -6, -2, 2, -11, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.insert(3), true);
  assertEquals([...tree.nlrValues()], [-4, -8, -10, -11, -6, 0, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -8, 0, -10, -6, -2, 2, -11, 3]);
});

Deno.test("[collections/RedBlackTree] remove rebalance root", () => {
  let values: number[] = [0];
  let tree: RedBlackTree<number> = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], []);

  values = [0, -1, 1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -1, 1]);
  assertEquals([...tree.lvlValues()], [0, -1, 1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [1, -1]);
  assertEquals([...tree.lvlValues()], [1, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [0, 1]);
  assertEquals([...tree.lvlValues()], [0, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [0, -1]);
  assertEquals([...tree.lvlValues()], [0, -1]);

  values = [0, -2, 2, -3, -1, 1, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, -3, -1, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3, -1, 1, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [1, -2, -3, -1, 2, 3]);
  assertEquals([...tree.lvlValues()], [1, -2, 2, -3, -1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [0, -1, -3, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [0, -1, 2, -3, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, -1, 3, 1]);
  assertEquals([...tree.lvlValues()], [0, -2, 3, -3, -1, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [0, -2, -1, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -1, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, -1, 2, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3, -1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, -1, 2, 1]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3, -1, 1]);

  values = [0, -2, 2, -3, -1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, -3, -1, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3, -1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-2, -3, 2, -1]);
  assertEquals([...tree.lvlValues()], [-2, -3, 2, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [0, -1, -3, 2]);
  assertEquals([...tree.lvlValues()], [0, -1, 2, -3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-2, -3, 0, -1]);
  assertEquals([...tree.lvlValues()], [-2, -3, 0, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [0, -2, -1, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3]);

  values = [0, -2, 2, 1, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, 1, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [1, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [1, -2, 2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [2, 0, 1, 3]);
  assertEquals([...tree.lvlValues()], [2, 0, 3, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [0, -2, 3, 1]);
  assertEquals([...tree.lvlValues()], [0, -2, 3, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2, 1]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, 1]);

  values = [0, -2, 2, -3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, -3, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3]);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [2, -2]);
  assertEquals([...tree.lvlValues()], [2, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [0, 2]);
  assertEquals([...tree.lvlValues()], [0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [0, -2]);
  assertEquals([...tree.lvlValues()], [0, -2]);

  values = [0, -2, 2, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, 3]);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [2, -2]);
  assertEquals([...tree.lvlValues()], [2, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [0, 2]);
  assertEquals([...tree.lvlValues()], [0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [0, -2]);
  assertEquals([...tree.lvlValues()], [0, -2]);

  values = [0, -2, 2, -3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, -3, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-2, -3, 2]);
  assertEquals([...tree.lvlValues()], [-2, -3, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [0, -3, 2]);
  assertEquals([...tree.lvlValues()], [0, -3, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-2, -3, 0]);
  assertEquals([...tree.lvlValues()], [-2, -3, 0]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2]);

  values = [0, -2, 2, -1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, -1, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-1, -2, 2]);
  assertEquals([...tree.lvlValues()], [-1, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [0, -1, 2]);
  assertEquals([...tree.lvlValues()], [0, -1, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-1, -2, 0]);
  assertEquals([...tree.lvlValues()], [-1, -2, 0]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2]);

  values = [0, -2, 2, 1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, 2, 1]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, 1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [1, -2, 2]);
  assertEquals([...tree.lvlValues()], [1, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [1, 0, 2]);
  assertEquals([...tree.lvlValues()], [1, 0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [0, -2, 1]);
  assertEquals([...tree.lvlValues()], [0, -2, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2]);

  values = [0, -2, 2, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [0, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [2, -2, 3]);
  assertEquals([...tree.lvlValues()], [2, -2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [2, 0, 3]);
  assertEquals([...tree.lvlValues()], [2, 0, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [0, -2, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2]);
});

Deno.test("[collections/RedBlackTree] remove rebalance left", () => {
  let values = [4, 5, 0];
  let tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, 5]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, 5]);

  values = [4, 5, 0, -1, 1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -1, 1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -1, 1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, 1, -1, 5]);
  assertEquals([...tree.lvlValues()], [4, 1, 5, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [4, 0, 1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -1]);

  values = [4, 5, 0, -2, 2, -3, -1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, -1, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -3, -1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, -2, -3, 2, -1, 5]);
  assertEquals([...tree.lvlValues()], [4, -2, 5, -3, 2, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -1, -3, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -1, 2, -3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, -2, -3, 0, -1, 5]);
  assertEquals([...tree.lvlValues()], [4, -2, 5, -3, 0, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -1, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -3]);

  values = [4, 5, 0, -2, 2, 1, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 1, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, 1, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, 1, -2, 2, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 1, 5, -2, 2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 2, 0, 1, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 2, 5, 0, 3, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 3, 1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 3, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, 1]);

  values = [4, 5, 0, -2, 2, -3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -3]);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, 2, -2, 5]);
  assertEquals([...tree.lvlValues()], [4, 2, 5, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 0, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(4), true);
  assertEquals([...tree.nlrValues()], [0, -2, 5, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 5, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(5), true);
  assertEquals([...tree.nlrValues()], [0, -2, 4, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 4, 2]);

  values = [4, 5, 0, -2, 2, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, 3]);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, 2, -2, 5]);
  assertEquals([...tree.lvlValues()], [4, 2, 5, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 0, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(4), true);
  assertEquals([...tree.nlrValues()], [0, -2, 5, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 5, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(5), true);
  assertEquals([...tree.nlrValues()], [0, -2, 4, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 4, 2]);

  values = [4, 5, 0, -2, 2, -3, -1, 1, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, -1, 2, 1, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -3, -1, 1, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, 1, -2, -3, -1, 2, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 1, 5, -2, 2, -3, -1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -1, -3, 2, 1, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -1, 2, -3, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, -1, 3, 1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 3, -3, -1, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -1, 2, 1, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -1, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, 2, 1, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -3, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, -1, 2, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -3, -1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, -1, 2, 1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -3, -1, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(4), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, -1, 2, 1, 5, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3, -1, 1, 5, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(5), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, -1, 2, 1, 4, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -3, -1, 1, 4, 3]);

  values = [4, 5, 0, -2, 2, -3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -3, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, -2, -3, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, -2, 5, -3, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -3, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -3, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, -2, -3, 0, 5]);
  assertEquals([...tree.lvlValues()], [4, -2, 5, -3, 0]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(4), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, 5, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 5, -3, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(5), true);
  assertEquals([...tree.nlrValues()], [0, -2, -3, 4, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 4, -3, 2]);

  values = [4, 5, 0, -2, 2, -1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, -1, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, -1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, -1, -2, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, -1, 5, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -1, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -1, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, -1, -2, 0, 5]);
  assertEquals([...tree.lvlValues()], [4, -1, 5, -2, 0]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(4), true);
  assertEquals([...tree.nlrValues()], [0, -2, -1, 5, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 5, -1, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(5), true);
  assertEquals([...tree.nlrValues()], [0, -2, -1, 4, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 4, -1, 2]);

  values = [4, 5, 0, -2, 2, 1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, 1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, 1, -2, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 1, 5, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 1, 0, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 1, 5, 0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 1, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(4), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2, 1, 5]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, 1, 5]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(5), true);
  assertEquals([...tree.nlrValues()], [0, -2, 2, 1, 4]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, 1, 4]);

  values = [4, 5, 0, -2, 2, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [4, 2, -2, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 2, 5, -2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [4, 2, 0, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 2, 5, 0, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 3, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [4, 0, -2, 2, 5]);
  assertEquals([...tree.lvlValues()], [4, 0, 5, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(4), true);
  assertEquals([...tree.nlrValues()], [0, -2, 3, 2, 5]);
  assertEquals([...tree.lvlValues()], [0, -2, 3, 2, 5]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(5), true);
  assertEquals([...tree.nlrValues()], [0, -2, 3, 2, 4]);
  assertEquals([...tree.lvlValues()], [0, -2, 3, 2, 4]);
});

Deno.test("[collections/RedBlackTree] remove rebalance right", () => {
  let values = [-4, -5, 0];
  let tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5]);

  values = [-4, -5, 0, -1, 1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -1, 1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -1, 1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 1, -1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 1, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, 1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -1]);

  values = [-4, -5, 0, -2, 2, -3, -1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, -1, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -3, -1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, -2, -3, 2, -1]);
  assertEquals([...tree.lvlValues()], [-4, -5, -2, -3, 2, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -1, -3, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -1, 2, -3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, -2, -3, 0, -1]);
  assertEquals([...tree.lvlValues()], [-4, -5, -2, -3, 0, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -1, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -3]);

  values = [-4, -5, 0, -2, 2, 1, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, 1, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 1, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 1, -2, 2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 2, 0, 1, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 2, 0, 3, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 3, 1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 3, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2, 1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, 1]);

  values = [-4, -5, 0, -2, 2, -3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -3]);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 2, -2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 2, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(-4), true);
  assertEquals([...tree.nlrValues()], [-2, -5, 0, 2]);
  assertEquals([...tree.lvlValues()], [-2, -5, 0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals(tree.remove(-5), true);
  assertEquals([...tree.nlrValues()], [0, -4, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -4, 2, -2]);

  values = [-4, -5, 0, -2, 2, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, 3]);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 2, -2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 2, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(-4), true);
  assertEquals([...tree.nlrValues()], [-2, -5, 0, 2]);
  assertEquals([...tree.lvlValues()], [-2, -5, 0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals(tree.remove(-5), true);
  assertEquals([...tree.nlrValues()], [0, -4, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -4, 2, -2]);

  values = [-4, -5, 0, -2, 2, -3, -1, 1, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, -1, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -3, -1, 1, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 1, -2, -3, -1, 2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 1, -2, 2, -3, -1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -1, -3, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -1, 2, -3, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, -1, 3, 1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 3, -3, -1, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -1, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -1, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -3, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, -1, 2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -3, -1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, -1, 2, 1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -3, -1, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-4), true);
  assertEquals([...tree.nlrValues()], [-3, -5, 0, -2, -1, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [-3, -5, 0, -2, 2, -1, 1, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-5), true);
  assertEquals([...tree.nlrValues()], [0, -2, -4, -3, -1, 2, 1, 3]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -4, -1, 1, 3, -3]);

  values = [-4, -5, 0, -2, 2, -3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -3, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, -2, -3, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, -2, -3, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -3, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -3, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, -2, -3, 0]);
  assertEquals([...tree.lvlValues()], [-4, -5, -2, -3, 0]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-3), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-4), true);
  assertEquals([...tree.nlrValues()], [-3, -5, 0, -2, 2]);
  assertEquals([...tree.lvlValues()], [-3, -5, 0, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-5), true);
  assertEquals([...tree.nlrValues()], [0, -3, -4, -2, 2]);
  assertEquals([...tree.lvlValues()], [0, -3, 2, -4, -2]);

  values = [-4, -5, 0, -2, 2, -1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, -1, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, -1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, -1, -2, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, -1, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -1, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -1, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, -1, -2, 0]);
  assertEquals([...tree.lvlValues()], [-4, -5, -1, -2, 0]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-1), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-4), true);
  assertEquals([...tree.nlrValues()], [-2, -5, 0, -1, 2]);
  assertEquals([...tree.lvlValues()], [-2, -5, 0, -1, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-5), true);
  assertEquals([...tree.nlrValues()], [0, -2, -4, -1, 2]);
  assertEquals([...tree.lvlValues()], [0, -2, 2, -4, -1]);

  values = [-4, -5, 0, -2, 2, 1];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2, 1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, 1]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 1, -2, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 1, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 1, 0, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 1, 0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 1]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 1]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(1), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-4), true);
  assertEquals([...tree.nlrValues()], [-2, -5, 1, 0, 2]);
  assertEquals([...tree.lvlValues()], [-2, -5, 1, 0, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-5), true);
  assertEquals([...tree.nlrValues()], [0, -4, -2, 2, 1]);
  assertEquals([...tree.lvlValues()], [0, -4, 2, -2, 1]);

  values = [-4, -5, 0, -2, 2, 3];
  tree = RedBlackTree.from(values);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2, 3]);
  assertEquals(tree.remove(0), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 2, -2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 2, -2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 2, 0, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 2, 0, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(2), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 3]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(3), true);
  assertEquals([...tree.nlrValues()], [-4, -5, 0, -2, 2]);
  assertEquals([...tree.lvlValues()], [-4, -5, 0, -2, 2]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-4), true);
  assertEquals([...tree.nlrValues()], [-2, -5, 2, 0, 3]);
  assertEquals([...tree.lvlValues()], [-2, -5, 2, 0, 3]);

  tree = RedBlackTree.from(values);
  assertEquals(tree.remove(-5), true);
  assertEquals([...tree.nlrValues()], [0, -4, -2, 2, 3]);
  assertEquals([...tree.lvlValues()], [0, -4, 2, -2, 3]);
});

Deno.test("[collections/RedBlackTree] README example", () => {
  const values = [3, 10, 13, 4, 6, 7, 1, 14];
  const tree = new RedBlackTree<number>();
  values.forEach((value) => tree.insert(value));
  assertEquals([...tree], [1, 3, 4, 6, 7, 10, 13, 14]);
  assertEquals(tree.min(), 1);
  assertEquals(tree.max(), 14);
  assertEquals(tree.find(42), null);
  assertEquals(tree.find(7), 7);
  assertEquals(tree.remove(42), false);
  assertEquals(tree.remove(7), true);
  assertEquals([...tree], [1, 3, 4, 6, 10, 13, 14]);

  const invertedTree = new RedBlackTree<number>(descend);
  values.forEach((value) => invertedTree.insert(value));
  assertEquals([...invertedTree], [14, 13, 10, 7, 6, 4, 3, 1]);
  assertEquals(invertedTree.min(), 14);
  assertEquals(invertedTree.max(), 1);
  assertEquals(invertedTree.find(42), null);
  assertEquals(invertedTree.find(7), 7);
  assertEquals(invertedTree.remove(42), false);
  assertEquals(invertedTree.remove(7), true);
  assertEquals([...invertedTree], [14, 13, 10, 6, 4, 3, 1]);

  const words = new RedBlackTree<string>((a, b) =>
    ascend(a.length, b.length) || ascend(a, b)
  );
  ["truck", "car", "helicopter", "tank", "train", "suv", "semi", "van"]
    .forEach((value) => words.insert(value));
  assertEquals([...words], [
    "car",
    "suv",
    "van",
    "semi",
    "tank",
    "train",
    "truck",
    "helicopter",
  ]);
  assertEquals(words.min(), "car");
  assertEquals(words.max(), "helicopter");
  assertEquals(words.find("scooter"), null);
  assertEquals(words.find("tank"), "tank");
  assertEquals(words.remove("scooter"), false);
  assertEquals(words.remove("tank"), true);
  assertEquals([...words], [
    "car",
    "suv",
    "van",
    "semi",
    "train",
    "truck",
    "helicopter",
  ]);
});
