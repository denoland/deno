// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
export function deepAssign(target: object, ...sources: object[]): object {
  for (let i = 0; i < sources.length; i++) {
    const source = sources[i];
    if (!source || typeof source !== `object`) {
      return;
    }
    Object.entries(source).forEach(([key, value]) => {
      if (value instanceof Date) {
        target[key] = new Date(value);
        return;
      }
      if (!value || typeof value !== `object`) {
        target[key] = value;
        return;
      }
      if (Array.isArray(value)) {
        target[key] = [];
      }
      // value is an Object
      if (typeof target[key] !== `object` || !target[key]) {
        target[key] = {};
      }
      deepAssign(target[key], value);
    });
  }
  return target;
}
