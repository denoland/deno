import { deferred } from "./deferred.ts";

export class DeadlineError extends Error {
  constructor() {
    super("Deadline");
    this.name = "DeadlineError";
  }
}

/**
 * Create a promise which will be rejected with DeadlineError when a given delay is exceeded.
 */
export function deadline<T>(p: Promise<T>, delay: number): Promise<T> {
  const d = deferred<never>();
  const t = setTimeout(() => d.reject(new DeadlineError()), delay);
  return Promise.race([p, d]).finally(() => clearTimeout(t));
}
