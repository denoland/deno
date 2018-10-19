// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "./test_util.ts";
import { CreateIterableIterator } from "./util";

test(function CreateIterableIteratorSuccess() {
  const list = [1, 2, 3, 4, 5];
  const iterators = new CreateIterableIterator(list);
  let idx = 0;
  for (const it of iterators) {
    assertEqual(it, list[idx++]);
  }
});