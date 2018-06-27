// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import { assertEqual, test } from "liltest";
import * as util from "./util";

test(async function test_removeSpaces() {
  const f = util.removeSpaces;
  assertEqual(f(""), "");
  assertEqual(f("   "), "");
  assertEqual(f(" \"  "), "\"  ");
  assertEqual(f(`  "x x x"   "x"  `), `"x x x""x"`);
  assertEqual(f(`  "x \\"x x"   "x"  `), `"x \\"x x""x"`);
  assertEqual(f(`  "x \\"x x"   "x"  `), `"x \\"x x""x"`);
  assertEqual(f(`  "x \\" y \\\\"   "x"  `), `"x \\" y \\\\""x"`);
});

test(async function test_one2ManyMap() {
  const map = new util.One2ManyMap();
  map.add("A", 1);
  map.add("A", 2);
  // `add()` should not work when map is locked.
  map.lock();
  map.add("A", 3);
  map.unlock();
  // forEachAfterLastSeparator should work before calling map.addSeparator.
  const a1 = [];
  map.forEachAfterLastSeparator("A", a1.push.bind(a1));
  assertEqual(a1, [2, 1]);
  // It should not iterate over previous elements after calling addSeparator().
  const a2 = [];
  map.addSeparator();
  map.forEachAfterLastSeparator("A", a2.push.bind(a2));
  assertEqual(a2, []);
  // Try to add new elements.
  const a3 = [];
  map.add("A", 4);
  map.forEachAfterLastSeparator("A", a3.push.bind(a3));
  assertEqual(a3, [4]);
  // clearKeyAfterLastSeparator() should work : )
  const a4 = [];
  map.clearKeyAfterLastSeparator("A");
  map.forEachAfterLastSeparator("A", a4.push.bind(a4));
  // it should not delete separator, so we expect an empty array.
  assertEqual(a4, []);
  // After calling removeLastSeparator() it should iterate over
  // previous elements.
  const a5 = [];
  map.removeLastSeparator();
  map.forEachAfterLastSeparator("A", a5.push.bind(a5));
  assertEqual(a5, [2, 1]);
});
