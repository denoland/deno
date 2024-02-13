// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2019 Allain Lalonde. All rights reserved. ISC License.
// This module is browser compatible.

/**
 * @module
 *
 * This module provides jest compatible expect assertion functionality.
 *
 * @example
 * ```ts
 * import { expect } from "https://deno.land/std@$STD_VERSION/expect/mod.ts";
 *
 * const x = 6 * 7;
 * expect(x).toEqual(42);
 * expect(x).not.toEqual(0);
 *
 * await expect(Promise.resolve(x)).resolves.toEqual(42);
 * ```
 *
 * Currently this module supports the following matchers:
 * - `toBe`
 * - `toEqual`
 * - `toStrictEqual`
 * - `toMatch`
 * - `toMatchObject`
 * - `toBeDefined`
 * - `toBeUndefined`
 * - `toBeNull`
 * - `toBeNaN`
 * - `toBeTruthy`
 * - `toBeFalsy`
 * - `toContain`
 * - `toContainEqual`
 * - `toHaveLength`
 * - `toBeGreaterThan`
 * - `toBeGreaterThanOrEqual`
 * - `toBeLessThan`
 * - `toBeLessThanOrEqual`
 * - `toBeCloseTo`
 * - `toBeInstanceOf`
 * - `toThrow`
 * - `toHaveProperty`
 * - `toHaveLength`
 *
 * Also this module supports the following mock related matchers:
 * - `toHaveBeenCalled`
 * - `toHaveBeenCalledTimes`
 * - `toHaveBeenCalledWith`
 * - `toHaveBeenLastCalledWith`
 * - `toHaveBeenNthCalledWith`
 * - `toHaveReturned`
 * - `toHaveReturnedTimes`
 * - `toHaveReturnedWith`
 * - `toHaveLastReturnedWith`
 * - `toHaveNthReturnedWith`
 *
 * The following matchers are not supported yet:
 * - `toMatchSnapShot`
 * - `toMatchInlineSnapShot`
 * - `toThrowErrorMatchingSnapShot`
 * - `toThrowErrorMatchingInlineSnapShot`
 *
 * The following asymmetric matchers are not supported yet:
 * - `expect.anything`
 * - `expect.any`
 * - `expect.arrayContaining`
 * - `expect.not.arrayContaining`
 * - `expect.closedTo`
 * - `expect.objectContaining`
 * - `expect.not.objectContaining`
 * - `expect.stringContaining`
 * - `expect.not.stringContaining`
 * - `expect.stringMatching`
 * - `expect.not.stringMatching`
 *
 * The following uitlities are not supported yet:
 * - `expect.assertions`
 * - `expect.hasAssertions`
 * - `expect.addEqualityTester`
 * - `expect.addSnapshotSerializer`
 * - `expect.extend`
 *
 * This module is largely inspired by [x/expect](https://github.com/allain/expect) module by Allain Lalonde.
 */

export { expect } from "./expect.ts";
export { fn } from "./fn.ts";
