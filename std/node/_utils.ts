export function notImplemented(msg?: string) {
  const message = msg ? `Not implemented: ${msg}` : "Not implemented";
  throw new Error(message);
}

// API helpers

export type MaybeNull<T> = T | null;
export type MaybeDefined<T> = T | undefined;
export type MaybeEmpty<T> = T | null | undefined;

export function intoCallbackAPI<T>(
  func: (...args: any[]) => Promise<T>,
  cb: MaybeEmpty<(err: MaybeNull<Error>, value: MaybeEmpty<T>) => void>,
  ...args: any[]
) {
  func(...args)
    .then(value => cb && cb(null, value))
    .catch(err => cb && cb(err, null));
}

export function intoCallbackAPIWithIntercept<T1, T2>(
  func: (...args: any[]) => Promise<T1>,
  interceptor: (v: T1) => T2,
  cb: MaybeEmpty<(err: MaybeNull<Error>, value: MaybeEmpty<T2>) => void>,
  ...args: any[]
) {
  func(...args)
    .then(value => cb && cb(null, interceptor(value)))
    .catch(err => cb && cb(err, null));
}
