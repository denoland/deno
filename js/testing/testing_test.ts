/*!
   Copyright 2018 Propel http://propel.site/.  All rights reserved.
   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

   http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
 */

import { test, assert, assertEqual, equal } from "./testing.ts";

test(function testingEqual() {
  assert(equal("world", "world"));
  assert(!equal("hello", "world"));
  assert(equal(5, 5));
  assert(!equal(5, 6));
  assert(equal(NaN, NaN));
  assert(equal({ hello: "world" }, { hello: "world" }));
  assert(!equal({ world: "hello" }, { hello: "world" }));
  assert(
    equal(
      { hello: "world", hi: { there: "everyone" } },
      { hello: "world", hi: { there: "everyone" } }
    )
  );
  assert(
    !equal(
      { hello: "world", hi: { there: "everyone" } },
      { hello: "world", hi: { there: "everyone else" } }
    )
  );
});

test(function testingAssertEqual() {
  const a = Object.create(null);
  a.b = "foo";
  assertEqual(a, a);
});

test(function testingAssertEqualActualUncoercable() {
  let didThrow = false;
  const a = Object.create(null);
  try {
    assertEqual(a, "bar");
  } catch (e) {
    didThrow = true;
    console.log(e.message);
    assert(e.message === "actual: [Cannot display] expected: bar");
  }
  assert(didThrow);
});

test(function testingAssertEqualExpectedUncoercable() {
  let didThrow = false;
  const a = Object.create(null);
  try {
    assertEqual("bar", a);
  } catch (e) {
    didThrow = true;
    console.log(e.message);
    assert(e.message === "actual: bar expected: [Cannot display]");
  }
  assert(didThrow);
});
