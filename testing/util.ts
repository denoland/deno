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

// tslint:disable-next-line:no-any
export function assertEqual(actual: any, expected: any, msg?: string) {
  if (!msg) { msg = `actual: ${actual} expected: ${expected}`; }
  if (!equal(actual, expected)) {
    console.error(
      "assertEqual failed. actual = ", actual, "expected =", expected);
    throw new Error(msg);
  }
}

export function assert(expr: boolean, msg = "") {
  if (!expr) {
    throw new Error(msg);
  }
}

// tslint:disable-next-line:no-any
export function equal(c: any, d: any): boolean {
  const seen = new Map();
  return (function compare(a, b) {
    if (a === b) {
      return true;
    }
    if (typeof a === "number" && typeof b === "number" &&
        isNaN(a) && isNaN(b)) {
      return true;
    }
    if (a && typeof a === "object" && b && typeof b === "object") {
      if (seen.get(a) === b) {
        return true;
      }
      if (Object.keys(a).length !== Object.keys(b).length) {
        return false;
      }
      for (const key in { ...a, ...b }) {
        if (!compare(a[key], b[key])) {
          return false;
        }
      }
      seen.set(a, b);
      return true;
    }
    return false;
  })(c, d);
}
