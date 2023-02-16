// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

type Fn<T> = (...args: unknown[]) => T;
export class FreeList<T> {
  name: string;
  ctor: Fn<T>;
  max: number;
  list: Array<T>;
  constructor(name: string, max: number, ctor: Fn<T>) {
    this.name = name;
    this.ctor = ctor;
    this.max = max;
    this.list = [];
  }

  alloc(): T {
    return this.list.length > 0
      ? this.list.pop()
      : Reflect.apply(this.ctor, this, arguments);
  }

  free(obj: T) {
    if (this.list.length < this.max) {
      this.list.push(obj);
      return true;
    }
    return false;
  }
}
