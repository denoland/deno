// Inspired by Elixir Guards:
// https://hexdocs.pm/elixir/guards.html
//
// Based on the latest ECMAScript standard (last updated Jun 4, 2020):
// See https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures
//
// Originally implemented by Slavomir Vojacek:
// https://github.com/hqoss/guards
//
// Copyright 2020, Slavomir Vojacek. All rights reserved. MIT license.

export const isUndefined = <T>(term: T | undefined): term is undefined => {
  return typeof term === "undefined";
};

export const isBoolean = <U>(term: boolean | U): term is boolean => {
  return typeof term === "boolean";
};

export const isNumber = <U>(term: number | U): term is number => {
  return typeof term === "number";
};

export const isString = <U>(term: string | U): term is string => {
  return typeof term === "string";
};

export const isBigInt = <U>(term: bigint | U): term is bigint => {
  return typeof term === "bigint";
};

export const isSymbol = <U>(term: symbol | U): term is symbol => {
  return typeof term === "symbol";
};
