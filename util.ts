
export function assert(cond: boolean, msg = "assert") {
  if (!cond) {
    throw Error(msg);
  }
}
