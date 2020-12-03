// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// TODO(ry) It'd be better to make Deferred a class that inherits from
// Promise, rather than an interface. This is possible in ES2016, however
// typescript produces broken code when targeting ES5 code.
// See https://github.com/Microsoft/TypeScript/issues/15202
// At the time of writing, the github issue is closed but the problem remains.
export class Deferred<T> extends Promise<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // deno-lint-ignore no-explicit-any
  reject: (reason?: any) => void;

  constructor(
    executor?: (
      resolve: (value?: T | PromiseLike<T>) => void,
      reject: (reason?: any) => void,
    ) => void,
  ) {
    if (executor instanceof Function) {
      return super(executor);
    }

    let methods: {
      resolve: (value?: T | PromiseLike<T>) => void;
      reject: (reason?: any) => void;
    };
    super((resolve, reject) => {
      methods = { resolve, reject };
    });
    this.resolve = methods!.resolve;
    this.reject = methods!.reject;
  }
}