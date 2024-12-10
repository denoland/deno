const a1;
const a2 = 1;
const a3, b3;
let a4 = 1;
var a4 = 1;

function foo1() {}
function foo2(a, b = 2) {}
function foo3(a, b?: number = 2): void {}
function foo4(a, ...rest: any[]) {}
function foo5({ a = 2 }) {}
function foo6([a, b]) {}
function foo7<T, U>(a: T, b: U) {}

export const e = 2;
export let e2 = 2;
export function e3() {}
export default () => {};

export * from "./foo.ts";

import * as imp1 from "./foo.ts";
import imp2 from "./foo.ts";
import { imp3, imp4 as imp5 } from "./foo.ts";
import json from "./json.json" with { type: "json" };
