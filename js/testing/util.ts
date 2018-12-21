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

export function assertEqual(actual: unknown, expected: unknown, msg?: string) {
  if (!equal(actual, expected)) {
    let actualString: string;
    let expectedString: string;
    try {
      actualString = String(actual);
    } catch (e) {
      actualString = "[Cannot display]";
    }
    try {
      expectedString = String(expected);
    } catch (e) {
      expectedString = "[Cannot display]";
    }
    console.error(
      "assertEqual failed. actual =",
      actualString,
      "expected =",
      expectedString
    );
    if (!msg) {
      msg = `actual: ${actualString} expected: ${expectedString}`;
    }
    throw new Error(msg);
  }
}

export function assert(expr: boolean, msg = "") {
  if (!expr) {
    throw new Error(msg);
  }
}

// TODO(ry) Use unknown here for parameters types.
// tslint:disable-next-line:no-any
export function equal(c: any, d: any): boolean {
  const seen = new Map();
  return (function compare(a, b) {
    if (Object.is(a, b)) {
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
