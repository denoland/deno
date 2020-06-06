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

export const isNull = <T>(term: T | null): term is null => {
  return term === null;
};

export const isFunction = <T extends Function, U>(term: T | U): term is T => {
  return typeof term === "function";
};

export const isObject = <T extends object, U>(
  term: T | U
): term is NonNullable<T> => {
  return !isNull(term) && typeof term === "object";
};

export const isArray = <T, U>(term: T[] | U): term is T[] => {
  return Array.isArray(term);
};

export const isMap = <K, V, U>(term: Map<K, V> | U): term is Map<K, V> => {
  return term instanceof Map;
};

export const isSet = <T, U>(term: Set<T> | U): term is Set<T> => {
  return term instanceof Set;
};

export const isWeakMap = <K extends object, V, U>(
  term: WeakMap<K, V> | U
): term is WeakMap<K, V> => {
  return term instanceof WeakMap;
};

export const isWeakSet = <T extends object, U>(
  term: WeakSet<T> | U
): term is WeakSet<T> => {
  return term instanceof WeakSet;
};

export const isDate = <U>(term: Date | U): term is Date => {
  return term instanceof Date;
};
