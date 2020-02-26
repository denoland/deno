import { assert } from "../../testing/asserts.ts";

function* portIterator(): IterableIterator<number> {
  // use 49152 ~ 65535
  let i = 49152;
  while (true) {
    yield i;
    i++;
    if (i > 65535) {
      i = 49152
    }
  }
}
const it = portIterator();
/** Obtain (maybe) safe port number for net tests */
export function usePort(): number {
  const { value } = it.next();
  assert(value != null);
  return value;
}
