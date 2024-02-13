// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../assert/mod.ts";
import { BinaryHeap } from "./binary_heap.ts";
import { ascend, descend } from "./comparators.ts";
import { Container, MyMath } from "./_test_utils.ts";

Deno.test("[collections/BinaryHeap] with default descend comparator", () => {
  const maxHeap = new BinaryHeap<number>();
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const expected: number[] = [100, 10, 9, 9, 9, 1, 0, -1, -9, -10, -100];
  let actual: number[] = [];

  assertEquals(maxHeap.length, 0);
  assertEquals(maxHeap.isEmpty(), true);
  assertEquals(maxHeap.peek(), undefined);
  for (let i = 0; i < values.length; i++) {
    assertEquals(maxHeap.push(values[i]), i + 1);
  }
  assertEquals(maxHeap.length, values.length);
  assertEquals(maxHeap.isEmpty(), false);
  while (!maxHeap.isEmpty()) {
    assertEquals(maxHeap.peek(), expected[actual.length]);
    actual.push(maxHeap.pop() as number);
    assertEquals(maxHeap.length, expected.length - actual.length);
    assertEquals(maxHeap.isEmpty(), actual.length === expected.length);
  }
  assertEquals(maxHeap.peek(), undefined);
  assertEquals(actual, expected);

  actual = [];
  assertEquals(maxHeap.push(...values), values.length);
  assertEquals(maxHeap.length, values.length);
  assertEquals(maxHeap.isEmpty(), false);
  assertEquals(maxHeap.peek(), expected[0]);
  for (const value of maxHeap) {
    actual.push(value);
    assertEquals(maxHeap.length, expected.length - actual.length);
    assertEquals(maxHeap.isEmpty(), actual.length === expected.length);
    assertEquals(maxHeap.peek(), expected[actual.length]);
  }
  assertEquals(actual, expected);
});

Deno.test("[collections/BinaryHeap] with ascend comparator", () => {
  const minHeap = new BinaryHeap<number>(ascend);
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 9, 9, 10, 100];
  let actual: number[] = [];

  assertEquals(minHeap.length, 0);
  assertEquals(minHeap.isEmpty(), true);
  assertEquals(minHeap.peek(), undefined);
  for (let i = 0; i < values.length; i++) {
    assertEquals(minHeap.push(values[i]), i + 1);
  }
  assertEquals(minHeap.length, values.length);
  assertEquals(minHeap.isEmpty(), false);
  while (!minHeap.isEmpty()) {
    assertEquals(minHeap.peek(), expected[actual.length]);
    actual.push(minHeap.pop() as number);
    assertEquals(minHeap.length, expected.length - actual.length);
    assertEquals(minHeap.isEmpty(), actual.length === expected.length);
  }
  assertEquals(minHeap.peek(), undefined);
  assertEquals(actual, expected);

  actual = [];
  assertEquals(minHeap.push(...values), values.length);
  assertEquals(minHeap.length, values.length);
  assertEquals(minHeap.isEmpty(), false);
  assertEquals(minHeap.peek(), expected[0]);
  for (const value of minHeap) {
    actual.push(value);
    assertEquals(minHeap.length, expected.length - actual.length);
    assertEquals(minHeap.isEmpty(), actual.length === expected.length);
    assertEquals(minHeap.peek(), expected[actual.length]);
  }
  assertEquals(actual, expected);
});

Deno.test("[collections/BinaryHeap] containing objects", () => {
  const heap = new BinaryHeap((
    a: Container,
    b: Container,
  ) => ascend(a.id, b.id));
  const ids: number[] = [-10, 9, -1, 100, 1, 0, -100, 10, -9];

  for (let i = 0; i < ids.length; i++) {
    const newContainer: Container = { id: ids[i], values: [] };
    assertEquals(heap.push(newContainer), i + 1);
    newContainer.values.push(i - 1, i, i + 1);
    assertEquals(heap.length, i + 1);
    assertEquals(heap.isEmpty(), false);
  }

  const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 10, 100];
  const expectedValue: number[] = [6, 0, 8, 2, 5, 4, 1, 7, 3];
  for (let i = 0; i < ids.length; i++) {
    assertEquals(heap.length, ids.length - i);
    assertEquals(heap.isEmpty(), false);

    const expectedContainer = {
      id: expected[i],
      values: [expectedValue[i] - 1, expectedValue[i], expectedValue[i] + 1],
    };
    assertEquals(heap.peek(), expectedContainer);
    assertEquals(heap.pop(), expectedContainer);
  }
  assertEquals(heap.length, 0);
  assertEquals(heap.isEmpty(), true);
});

Deno.test("[collections/BinaryHeap] from Iterable", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const originalValues: number[] = Array.from(values);
  const expected: number[] = [100, 10, 9, 9, 9, 1, 0, -1, -9, -10, -100];
  let heap = BinaryHeap.from(values);
  assertEquals(values, originalValues);
  assertEquals([...heap], expected);

  heap = BinaryHeap.from(values, { compare: ascend });
  assertEquals(values, originalValues);
  assertEquals([...heap].reverse(), expected);

  heap = BinaryHeap.from(values, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...heap], expected.map((v: number) => 2 * v));

  const math = new MyMath();
  heap = BinaryHeap.from(values, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals(values, originalValues);
  assertEquals([...heap], expected.map((v: number) => 3 * v));

  heap = BinaryHeap.from(values, {
    compare: ascend,
    map: (v: number) => 2 * v,
  });
  assertEquals(values, originalValues);
  assertEquals([...heap].reverse(), expected.map((v: number) => 2 * v));

  heap = BinaryHeap.from(values, {
    compare: ascend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals(values, originalValues);
  assertEquals([...heap].reverse(), expected.map((v: number) => 3 * v));
});

Deno.test("[collections/BinaryHeap] from BinaryHeap with default descend comparator", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const expected: number[] = [100, 10, 9, 9, 9, 1, 0, -1, -9, -10, -100];
  const maxHeap = new BinaryHeap<number>();
  maxHeap.push(...values);
  let heap = BinaryHeap.from(maxHeap);
  assertEquals([...maxHeap], expected);
  assertEquals([...heap], expected);

  maxHeap.push(...values);
  heap = BinaryHeap.from(maxHeap, { compare: ascend });
  assertEquals([...maxHeap], expected);
  assertEquals([...heap].reverse(), expected);

  maxHeap.push(...values);
  heap = BinaryHeap.from(maxHeap, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...maxHeap], expected);
  assertEquals([...heap], expected.map((v: number) => 2 * v));

  const math = new MyMath();
  maxHeap.push(...values);
  heap = BinaryHeap.from(maxHeap, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...maxHeap], expected);
  assertEquals([...heap], expected.map((v: number) => 3 * v));

  maxHeap.push(...values);
  heap = BinaryHeap.from(maxHeap, {
    compare: ascend,
    map: (v: number) => 2 * v,
  });
  assertEquals([...maxHeap], expected);
  assertEquals([...heap].reverse(), expected.map((v: number) => 2 * v));

  maxHeap.push(...values);
  heap = BinaryHeap.from(maxHeap, {
    compare: ascend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...maxHeap], expected);
  assertEquals([...heap].reverse(), expected.map((v: number) => 3 * v));
});

Deno.test("[collections/BinaryHeap] from BinaryHeap with ascend comparator", () => {
  const values: number[] = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const expected: number[] = [-100, -10, -9, -1, 0, 1, 9, 9, 9, 10, 100];
  const minHeap = new BinaryHeap<number>(ascend);
  minHeap.push(...values);
  let heap = BinaryHeap.from(minHeap);
  assertEquals([...minHeap], expected);
  assertEquals([...heap], expected);

  minHeap.push(...values);
  heap = BinaryHeap.from(minHeap, { compare: descend });
  assertEquals([...minHeap], expected);
  assertEquals([...heap].reverse(), expected);

  minHeap.push(...values);
  heap = BinaryHeap.from(minHeap, {
    map: (v: number) => 2 * v,
  });
  assertEquals([...minHeap], expected);
  assertEquals([...heap], expected.map((v: number) => 2 * v));

  const math = new MyMath();
  minHeap.push(...values);
  heap = BinaryHeap.from(minHeap, {
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...minHeap], expected);
  assertEquals([...heap], expected.map((v: number) => 3 * v));

  minHeap.push(...values);
  heap = BinaryHeap.from(minHeap, {
    compare: descend,
    map: (v: number) => 2 * v,
  });
  assertEquals([...minHeap], expected);
  assertEquals([...heap].reverse(), expected.map((v: number) => 2 * v));

  minHeap.push(...values);
  heap = BinaryHeap.from(minHeap, {
    compare: descend,
    map: function (this: MyMath, v: number) {
      return this.multiply(3, v);
    },
    thisArg: math,
  });
  assertEquals([...minHeap], expected);
  assertEquals([...heap].reverse(), expected.map((v: number) => 3 * v));
});

Deno.test("[collections/BinaryHeap] edge case 1", () => {
  const minHeap = new BinaryHeap<number>(ascend);
  minHeap.push(4, 2, 8, 1, 10, 7, 3, 6, 5);
  assertEquals(minHeap.pop(), 1);
  minHeap.push(9);

  const expected = [2, 3, 4, 5, 6, 7, 8, 9, 10];
  assertEquals([...minHeap], expected);
});

Deno.test("[collections/BinaryHeap] edge case 2", () => {
  interface Point {
    x: number;
    y: number;
  }
  const minHeap = new BinaryHeap<Point>((a, b) => ascend(a.x, b.x));
  minHeap.push({ x: 0, y: 1 }, { x: 0, y: 2 }, { x: 0, y: 3 });

  const expected = [{ x: 0, y: 1 }, { x: 0, y: 3 }, { x: 0, y: 2 }];
  assertEquals([...minHeap], expected);
});

Deno.test("[collections/BinaryHeap] edge case 3", () => {
  interface Point {
    x: number;
    y: number;
  }
  const minHeap = new BinaryHeap<Point>((a, b) => ascend(a.x, b.x));
  minHeap.push(
    { x: 0, y: 1 },
    { x: 1, y: 2 },
    { x: 1, y: 3 },
    { x: 2, y: 4 },
    { x: 2, y: 5 },
    { x: 2, y: 6 },
    { x: 2, y: 7 },
  );

  const expected = [
    { x: 0, y: 1 },
    { x: 1, y: 2 },
    { x: 1, y: 3 },
    { x: 2, y: 5 },
    { x: 2, y: 4 },
    { x: 2, y: 6 },
    { x: 2, y: 7 },
  ];
  assertEquals([...minHeap], expected);
});

Deno.test("[collections/BinaryHeap] README example", () => {
  const maxHeap = new BinaryHeap<number>();
  maxHeap.push(4, 1, 3, 5, 2);
  assertEquals(maxHeap.peek(), 5);
  assertEquals(maxHeap.pop(), 5);
  assertEquals([...maxHeap], [4, 3, 2, 1]);
  assertEquals([...maxHeap], []);

  const minHeap = new BinaryHeap<number>(ascend);
  minHeap.push(4, 1, 3, 5, 2);
  assertEquals(minHeap.peek(), 1);
  assertEquals(minHeap.pop(), 1);
  assertEquals([...minHeap], [2, 3, 4, 5]);
  assertEquals([...minHeap], []);

  const words = new BinaryHeap<string>((a, b) => descend(a.length, b.length));
  words.push("truck", "car", "helicopter", "tank");
  assertEquals(words.peek(), "helicopter");
  assertEquals(words.pop(), "helicopter");
  assertEquals([...words], ["truck", "tank", "car"]);
  assertEquals([...words], []);
});

Deno.test("[collections/BinaryHeap] .toArray() and .clear()", () => {
  const values = [-10, 9, -1, 100, 9, 1, 0, 9, -100, 10, -9];
  const maxHeap = new BinaryHeap<number>();
  maxHeap.push(...values);
  assert(maxHeap.toArray().every((value) => values.includes(value)));
  maxHeap.clear();
  assertEquals(maxHeap.toArray(), []);
});
