// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrows,
} from "../assert/mod.ts";
import { BinarySearchTree } from "./binary_search_tree.ts";
import { ascend, descend } from "./comparators.ts";

class MyMath {
  multiply(a: number, b: number): number {
    return a * b;
  }
}

interface Container {
  id: number;
  values: number[];
}

Deno.test("[collections/BinarySearchTree] with default ascend comparator", () => {
  const trees: BinarySearchTree<number>[] = [
    new BinarySearchTree(),
    new BinarySearchTree(),
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
    [-10, -100, 9, -1, -9, 1, 0, 100, 10],
  );
  assertEquals(
    [...trees[1].nlrValues()],
    [-9, -100, -10, 10, 0, -1, 1, 9, 100],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].lrnValues()],
    [-100, -9, 0, 1, -1, 10, 100, 9, -10],
  );
  assertEquals(
    [...trees[1].lrnValues()],
    [-10, -100, -1, 9, 1, 0, 100, 10, -9],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].lvlValues()],
    [-10, -100, 9, -1, 100, -9, 1, 10, 0],
  );
  assertEquals(
    [...trees[1].lvlValues()],
    [-9, -100, 10, -10, 0, 100, -1, 1, 9],
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

Deno.test("[collections/BinarySearchTree] with descend comparator", () => {
  const trees: BinarySearchTree<number>[] = [
    new BinarySearchTree(descend),
    new BinarySearchTree(descend),
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
    [-10, 9, 100, 10, -1, 1, 0, -9, -100],
  );
  assertEquals(
    [...trees[1].nlrValues()],
    [-9, 10, 100, 0, 1, 9, -1, -100, -10],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].lrnValues()],
    [10, 100, 0, 1, -9, -1, 9, -100, -10],
  );
  assertEquals(
    [...trees[1].lrnValues()],
    [100, 9, 1, -1, 0, 10, -10, -100, -9],
  );
  for (let i = 0; i < 2; i++) {
    assertEquals(trees[i].size, values.length);
    assertEquals(trees[i].isEmpty(), false);
  }

  assertEquals(
    [...trees[0].lvlValues()],
    [-10, 9, -100, 100, -1, 10, 1, -9, 0],
  );
  assertEquals(
    [...trees[1].lvlValues()],
    [-9, 10, -100, 100, 0, -10, 1, -1, 9],
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

Deno.test("[collections/BinarySearchTree] containing objects", () => {
  const tree: BinarySearchTree<Container> = new BinarySearchTree((
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

Deno.test("[collections/BinarySearchTree] from Iterable", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const originalValues: number[] = Array.from(values);
  const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 10, 100];
  let tree: BinarySearchTree<number> = BinarySearchTree.from(values);
  assertEquals(values, originalValues);
  assertEquals([...tree], expected);
  assertEquals([...tree.nlrValues()], [-10, -100, 9, -1, -9, 1, 0, 100, 10]);
  assertEquals([...tree.lvlValues()], [-10, -100, 9, -1, 100, -9, 1, 10, 0]);

  tree = BinarySearchTree.from(values, { compare: descend });
  assertEquals(values, originalValues);
  assertEquals([...tree].reverse(), expected);
  assertEquals([...tree.nlrValues()], [-10, 9, 100, 10, -1, 1, 0, -9, -100]);
  assertEquals([...tree.lvlValues()], [-10, 9, -100, 100, -1, 10, 1, -9, 0]);

  tree = BinarySearchTree.from(values, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...tree], expected.map((v: number) => 2 * v));
  assertEquals([...tree.nlrValues()], [-20, -200, 18, -2, -18, 2, 0, 200, 20]);
  assertEquals([...tree.lvlValues()], [-20, -200, 18, -2, 200, -18, 2, 20, 0]);

  const math = new MyMath();
  tree = BinarySearchTree.from(values, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals(values, originalValues);
  assertEquals([...tree], expected.map((v: number) => 3 * v));
  assertEquals([...tree.nlrValues()], [-30, -300, 27, -3, -27, 3, 0, 300, 30]);
  assertEquals([...tree.lvlValues()], [-30, -300, 27, -3, 300, -27, 3, 30, 0]);

  tree = BinarySearchTree.from(values, {
    compare: descend,
    map: (v: number) => 2 * v,
  });
  assertEquals(values, originalValues);
  assertEquals([...tree].reverse(), expected.map((v: number) => 2 * v));
  assertEquals([...tree.nlrValues()], [-20, 18, 200, 20, -2, 2, 0, -18, -200]);
  assertEquals([...tree.lvlValues()], [-20, 18, -200, 200, -2, 20, 2, -18, 0]);

  tree = BinarySearchTree.from(values, {
    compare: descend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals(values, originalValues);
  assertEquals([...tree].reverse(), expected.map((v: number) => 3 * v));
  assertEquals([...tree.nlrValues()], [-30, 27, 300, 30, -3, 3, 0, -27, -300]);
  assertEquals([...tree.lvlValues()], [-30, 27, -300, 300, -3, 30, 3, -27, 0]);
});

Deno.test("[collections/BinarySearchTree] from BinarySearchTree with default ascend comparator", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 10, 100];
  const originalTree: BinarySearchTree<number> = new BinarySearchTree();
  for (const value of values) originalTree.insert(value);
  let tree: BinarySearchTree<number> = BinarySearchTree.from(originalTree);
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected);
  assertEquals([...tree.nlrValues()], [...originalTree.nlrValues()]);
  assertEquals([...tree.lvlValues()], [...originalTree.lvlValues()]);

  tree = BinarySearchTree.from(originalTree, { compare: descend });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected);
  assertEquals([...tree.nlrValues()], expected);
  assertEquals([...tree.lvlValues()], expected);

  tree = BinarySearchTree.from(originalTree, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected.map((v: number) => 2 * v));

  const math = new MyMath();
  tree = BinarySearchTree.from(originalTree, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected.map((v: number) => 3 * v));

  tree = BinarySearchTree.from(originalTree, {
    compare: descend,
    map: (v: number) => 2 * v,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected.map((v: number) => 2 * v));

  tree = BinarySearchTree.from(originalTree, {
    compare: descend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected.map((v: number) => 3 * v));
});

Deno.test("[collections/BinarySearchTree] from BinarySearchTree with descend comparator", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const expected: number[] = [100, 10, 9, 1, 0, -1, -9, -10, -100];
  const originalTree = new BinarySearchTree<number>(descend);
  for (const value of values) originalTree.insert(value);
  let tree: BinarySearchTree<number> = BinarySearchTree.from(originalTree);
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected);
  assertEquals([...tree.nlrValues()], [...originalTree.nlrValues()]);
  assertEquals([...tree.lvlValues()], [...originalTree.lvlValues()]);

  tree = BinarySearchTree.from(originalTree, { compare: ascend });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected);
  assertEquals([...tree.nlrValues()], expected);
  assertEquals([...tree.lvlValues()], expected);

  tree = BinarySearchTree.from(originalTree, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected.map((v: number) => 2 * v));

  const math = new MyMath();
  tree = BinarySearchTree.from(originalTree, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree], expected.map((v: number) => 3 * v));

  tree = BinarySearchTree.from(originalTree, {
    compare: ascend,
    map: (v: number) => 2 * v,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected.map((v: number) => 2 * v));

  tree = BinarySearchTree.from(originalTree, {
    compare: ascend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...originalTree], expected);
  assertEquals([...tree].reverse(), expected.map((v: number) => 3 * v));
});

Deno.test("[collections/BinarySearchTree] README example", () => {
  const values = [3, 10, 13, 4, 6, 7, 1, 14];
  const tree = new BinarySearchTree<number>();
  values.forEach((value) => tree.insert(value));
  assertEquals([...tree], [1, 3, 4, 6, 7, 10, 13, 14]);
  assertEquals(tree.min(), 1);
  assertEquals(tree.max(), 14);
  assertEquals(tree.find(42), null);
  assertEquals(tree.find(7), 7);
  assertEquals(tree.remove(42), false);
  assertEquals(tree.remove(7), true);
  assertEquals([...tree], [1, 3, 4, 6, 10, 13, 14]);

  const invertedTree = new BinarySearchTree<number>(descend);
  values.forEach((value) => invertedTree.insert(value));
  assertEquals([...invertedTree], [14, 13, 10, 7, 6, 4, 3, 1]);
  assertEquals(invertedTree.min(), 14);
  assertEquals(invertedTree.max(), 1);
  assertEquals(invertedTree.find(42), null);
  assertEquals(invertedTree.find(7), 7);
  assertEquals(invertedTree.remove(42), false);
  assertEquals(invertedTree.remove(7), true);
  assertEquals([...invertedTree], [14, 13, 10, 6, 4, 3, 1]);

  const words = new BinarySearchTree<string>((a, b) =>
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

Deno.test("[collections/BinarySearchTree] nully .max() and .clear()", () => {
  const tree = BinarySearchTree.from([1]);
  assert(!tree.isEmpty());
  tree.clear();
  assert(tree.isEmpty());
  assertEquals(tree.max(), null);
});

Deno.test("[collections/BinarySearchTree] .rotateNode()", () => {
  class MyTree<T> extends BinarySearchTree<T> {
    rotateNode2() {
      super.rotateNode(this.root!, "right");
    }
  }
  const tree = new MyTree();
  tree.insert(1);
  assertThrows(() => tree.rotateNode2());
});
