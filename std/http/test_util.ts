import { assert } from "../testing/asserts.ts";

function* portIterator(): IterableIterator<number> {
  // use 55001 ~ 65535 (rest (49152~55000) are for cli/js)
  let i = 55001;
  while (true) {
    yield i;
    i++;
    if (i > 65535) {
      i = 55001;
    }
  }
}
const it = portIterator();
/** Obtain (maybe) safe port number for net tests */
export function randomPort(): number {
  const { value } = it.next();
  assert(value != null);
  return value;
}
