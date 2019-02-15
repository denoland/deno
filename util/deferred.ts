// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

export type Deferred<T = any, R = Error> = {
  promise: Promise<T>;
  resolve: (t?: T) => void;
  reject: (r?: R) => void;
  readonly handled: boolean;
};

/** Create deferred promise that can be resolved and rejected by outside */
export function defer<T>(): Deferred<T> {
  let handled = false;
  let resolve;
  let reject;
  const promise = new Promise<T>((res, rej) => {
    resolve = r => {
      handled = true;
      res(r);
    };
    reject = r => {
      handled = true;
      rej(r);
    };
  });
  return {
    promise,
    resolve,
    reject,
    get handled() {
      return handled;
    }
  };
}

export function isDeferred(x): x is Deferred {
  return (
    typeof x === "object" &&
    x.promise instanceof Promise &&
    typeof x["resolve"] === "function" &&
    typeof x["reject"] === "function"
  );
}
