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

import { isNumber } from "./primitives.ts";
import { isArray } from "./special.ts";

export const isNonEmptyArray = <T, U>(term: T[] | U): term is T[] => {
  return isArray(term) && term.length > 0;
};

export const isValidNumber = <U>(term: number | U): term is number => {
  return isNumber(term) && !Number.isNaN(term);
};

export const isInteger = <U>(term: number | U): term is number => {
  return isValidNumber(term) && Number.isInteger(term);
};

export const isPositiveInteger = <U>(term: number | U): term is number => {
  return isInteger(term) && term > 0;
};

export const isNonNegativeInteger = <U>(term: number | U): term is number => {
  return isInteger(term) && term >= 0;
};

export const isNegativeInteger = <U>(term: number | U): term is number => {
  return isInteger(term) && term < 0;
};
