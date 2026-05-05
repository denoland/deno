// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file prefer-primordials

(function () {
const ArrayIsArray = Array.isArray;
const ArrayPrototypeFilter = (that, ...args) => that.filter(...args);
const ArrayPrototypeForEach = (that, ...args) => that.forEach(...args);
const ArrayPrototypeIncludes = (that, ...args) => that.includes(...args);
const ArrayPrototypeJoin = (that, ...args) => that.join(...args);
const ArrayPrototypePush = (that, ...args) => that.push(...args);
const ArrayPrototypeSlice = (that, ...args) => that.slice(...args);
const ArrayPrototypeSome = (that, ...args) => that.some(...args);
const ArrayPrototypeSort = (that, ...args) => that.sort(...args);
const ArrayPrototypeUnshift = (that, ...args) => that.unshift(...args);
const BigInt = globalThis.BigInt;
const ObjectAssign = Object.assign;
const ObjectCreate = Object.create;
const ObjectHasOwn = Object.hasOwn;
const RegExpPrototypeTest = (that, ...args) => that.test(...args);
const RegExpPrototypeExec = RegExp.prototype.exec;
const StringFromCharCode = String.fromCharCode;
const StringPrototypeCharCodeAt = (that, ...args) => that.charCodeAt(...args);
const StringPrototypeEndsWith = (that, ...args) => that.endsWith(...args);
const StringPrototypeIncludes = (that, ...args) => that.includes(...args);
const StringPrototypeReplace = (that, ...args) => that.replace(...args);
const StringPrototypeSlice = (that, ...args) => that.slice(...args);
const StringPrototypeSplit = (that, ...args) => that.split(...args);
const StringPrototypeStartsWith = (that, ...args) => that.startsWith(...args);
const StringPrototypeToUpperCase = (that) => that.toUpperCase();

return {
  ArrayIsArray,
  ArrayPrototypeFilter,
  ArrayPrototypeForEach,
  ArrayPrototypeIncludes,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSome,
  ArrayPrototypeSort,
  ArrayPrototypeUnshift,
  BigInt,
  ObjectAssign,
  ObjectCreate,
  ObjectHasOwn,
  RegExpPrototypeTest,
  RegExpPrototypeExec,
  StringFromCharCode,
  StringPrototypeCharCodeAt,
  StringPrototypeEndsWith,
  StringPrototypeIncludes,
  StringPrototypeReplace,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeToUpperCase,
};
})();
