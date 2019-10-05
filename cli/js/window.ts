// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// (0, eval) is indirect eval.
// See the links below for details:
// - https://stackoverflow.com/a/14120023
// - https://tc39.github.io/ecma262/#sec-performeval (spec)
export const window = (0, eval)("this");
// TODO: The above should be replaced with globalThis
// when the globalThis proposal goes to stage 4
// See https://github.com/tc39/proposal-global
