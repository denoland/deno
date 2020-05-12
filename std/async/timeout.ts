import { assert } from "../testing/asserts.ts";
import { deferred } from "./deferred.ts";

export class TimeoutError extends Error {
  constructor(msg = "Operation timed out") {
    super(msg);
  }
}

export function letTimeout<T>(p: Promise<T>, timeoutMs?: number): Promise<T> {
  // noop for no timeout
  if (timeoutMs == null) {
    return p;
  }
  assert(timeoutMs > 0, "timeout must be greater than zero");
  const d = deferred<T>();
  const timer = setTimeout(() => {
    d.reject(new TimeoutError());
  }, timeoutMs);
  p.then(d.resolve)
    .catch(d.reject)
    .finally(() => {
      clearTimeout(timer);
    });
  return d;
}

export function timeoutReader(r: Deno.Reader, timeoutMs: number): Deno.Reader {
  return {
    read(p: Uint8Array): Promise<number | null> {
      return letTimeout(r.read(p), timeoutMs);
    },
  };
}
