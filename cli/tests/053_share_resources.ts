interface Deferred<T> extends Promise<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  reject: (reason?: any) => void;
}
function deferred<T>(): Deferred<T> {
  let methods;
  const promise = new Promise<T>((resolve, reject): void => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods)! as Deferred<T>;
}

interface TaggedYieldedValue<T> {
  iterator: AsyncIterableIterator<T>;
  value: T;
}

const _f = Deno.openSync("./shared_resource_worker.ts");
console.log(Deno.resources());

const w0 = new Worker("./shared_resource_worker.ts", {
  shareResources: true
});
const w0Deferred = deferred();
// eslint-disable-next-line @typescript-eslint/no-explicit-any
w0.onmessage = (_e: any): void => {
  w0Deferred.resolve();
};
await w0Deferred;

const w1 = new Worker("./shared_resource_worker.ts");
const w1Deferred = deferred();
// eslint-disable-next-line @typescript-eslint/no-explicit-any
w1.onmessage = (_e: any): void => {
  w1Deferred.resolve();
};
await w1Deferred;

Deno.exit(0);
