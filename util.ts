// If you use the eval function indirectly, by invoking it via a reference
// other than eval, as of ECMAScript 5 it works in the global scope rather than
// the local scope. This means, for instance, that function declarations create
// global functions, and that the code being evaluated doesn't have access to
// local variables within the scope where it's being called.
export const globalEval = eval;

// A reference to the global object.
const _global = globalEval("this");

const print = V8Worker2.print;

_global["console"] = {
  // tslint:disable-next-line:no-any
  log(...args: any[]): void {
    print(stringifyArgs(args));
  },

  // tslint:disable-next-line:no-any
  error(...args: any[]): void {
    print("ERROR: " + stringifyArgs(args));
  }
};

// tslint:disable-next-line:no-any
function stringifyArgs(args: any[]): string {
  const out: string[] = [];
  for (const a of args) {
    if (typeof a === "string") {
      out.push(a);
    } else {
      out.push(JSON.stringify(a));
    }
  }
  return out.join(" ");
}

export function assert(cond: boolean, msg = "") {
  if (!cond) {
    throw Error("Assertion failed. " + msg);
  }
}
