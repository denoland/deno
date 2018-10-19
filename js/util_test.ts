// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";
import { CreateIterableIterator } from "./util";

test(function CreateIterableIteratorSuccess() {
  const list = [1, 2, 3, 4, 5];
  const listIterators = new CreateIterableIterator(list.values());
  let idx = 0;
  for (const it of listIterators) {
    assertEqual(it, list[idx++]);
  }
  const obj = {
    a: "foo",
    b: "bar",
    c: "baz"
  };
  const list1 = [];
  const keys = Object.keys(obj);
  keys.forEach(key => list1.push([key, obj[key]]));
  const objectIterators = new CreateIterableIterator(list1.values());
  for (const it of objectIterators) {
    const [key, value] = it;
    assert(key in obj);
    assertEqual(value, obj[key]);
  }
});