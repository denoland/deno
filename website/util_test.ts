// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as util from "./util";
import { assertEqual, test } from "liltest";

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
