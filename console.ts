const print = V8Worker2.print;

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

export const globalConsole = {
  // tslint:disable-next-line:no-any
  log(...args: any[]): void {
    print(stringifyArgs(args));
  },

  // tslint:disable-next-line:no-any
  error(...args: any[]): void {
    print("ERROR: " + stringifyArgs(args));
  },

  // tslint:disable-next-line:no-any
  assert(condition: boolean, ...args: any[]): void {
    if (!condition) {
      throw new Error("Assertion failed: " + stringifyArgs(args));
    }
  }
};
