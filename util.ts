// If you use the eval function indirectly, by invoking it via a reference
// other than eval, as of ECMAScript 5 it works in the global scope rather than
// the local scope. This means, for instance, that function declarations create
// global functions, and that the code being evaluated doesn't have access to
// local variables within the scope where it's being called.
const globalEval = eval;

// A reference to the global object.
const _global = globalEval("this");

_global["console"] = {
  log(...args: any[]): void {
    const out: string[] = [];
    for (let a of args) {
      if (typeof(a) === "string") {
        out.push(a);
      } else {
        out.push(JSON.stringify(a));
      }
    }
    V8Worker2.print(out.join(" "));
  }
};
