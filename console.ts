class ConsoleContext {
  seen = new Set<{}>();
}

// tslint:disable-next-line:no-any
function stringify(ctx: ConsoleContext, value: any): string {
  switch (typeof value) {
    case "string":
      return `"${value}"`;
    case "number":
    case "boolean":
    case "undefined":
      return String(value);
    case "function":
      return "[Function]";
    case "object":
      if (value === null) {
        return "null";
      }
      if (ctx.seen.has(value)) {
        return "[Circular]";
      }

      ctx.seen.add(value);

      const keys = Object.keys(value);
      const keyStrings = [];
      for (const key of keys) {
        keyStrings.push(`${key}: ${stringify(ctx, value[key])}`);
      }

      ctx.seen.delete(value);

      if (keyStrings.length === 0) {
        return "{}";
      }

      return `{ ${keyStrings.join(", ")} }`;
    default:
      return "[Not Implemented]";
  }
}

// tslint:disable-next-line:no-any
function stringifyArgs(args: any[]): string {
  const out: string[] = [];
  for (const a of args) {
    if (typeof a === "string") {
      out.push(a);
    } else {
      out.push(stringify(new ConsoleContext(), a));
    }
  }
  return out.join(" ");
}

const print = V8Worker2.print;

export class Console {
  // tslint:disable-next-line:no-any
  log(...args: any[]): void {
    print(stringifyArgs(args));
  }

  debug = this.log;
  info = this.log;
  dirxml = this.log;

  // tslint:disable-next-line:no-any
  warn(...args: any[]): void {
    print("ERROR: " + stringifyArgs(args));
  }

  error = this.warn;

  // tslint:disable-next-line:no-any
  assert(condition: boolean, ...args: any[]): void {
    if (!condition) {
      throw new Error("Assertion failed: " + stringifyArgs(args));
    }
  }
}
