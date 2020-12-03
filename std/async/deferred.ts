// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export class Deferred<T> extends Promise<T> {
  resolve!: (value: T | PromiseLike<T>) => void;
  // deno-lint-ignore no-explicit-any
  reject!: (reason?: any) => void;

  //it's necessary to pass an executor so that
  //.then() can function properly on the returned deferred object
  constructor(
    executor?: (
      resolve: (value: T | PromiseLike<T>) => void,
      // deno-lint-ignore no-explicit-any
      reject: (reason?: any) => void,
    ) => void,
  ) {
    //For .then() to function properly
    if (executor instanceof Function) {
      return new Promise<T>(executor) as Deferred<T>;
    }

    let methods: {
      resolve: (value: T | PromiseLike<T>) => void;
      // deno-lint-ignore no-explicit-any
      reject: (reason?: any) => void;
    };

    //capture references to resolve and reject functions
    super((resolve, reject) => {
      methods = { resolve, reject };
    });

    this.resolve = methods!.resolve;
    this.reject = methods!.reject;
  }
}
