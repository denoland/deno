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

import { test } from "./testing.ts";
import { assert } from "./util.ts";
import * as util from "./util.ts";

test(async function util_equal() {
  assert(util.equal("world", "world"));
  assert(!util.equal("hello", "world"));
  assert(util.equal(5, 5));
  assert(!util.equal(5, 6));
  assert(util.equal(NaN, NaN));
  assert(util.equal({ hello: "world" }, { hello: "world" }));
  assert(!util.equal({ world: "hello" }, { hello: "world" }));
  assert(util.equal({ hello: "world", hi: { there: "everyone" } },
                    { hello: "world", hi: { there: "everyone" } }));
  assert(!util.equal({ hello: "world", hi: { there: "everyone" } },
                    { hello: "world", hi: { there: "everyone else" } }));
});
