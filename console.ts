// tslint:disable-next-line:no-any
type ConsoleContext = Set<any>;

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

      if (ctx.has(value)) {
        return "[Circular]";
      }

      ctx.add(value);
      const valStrings = [];

      if (Array.isArray(value)) {
        for (const el of value) {
          valStrings.push(stringify(ctx, el));
        }
        
        ctx.delete(value);
        
        if (valStrings.length === 0) {
          return "[]";
        }
        return `[${valStrings.join(", ")}]`;
      } else {
        for (const key of Object.keys(value)) {
          valStrings.push(`${key}: ${stringify(ctx, value[key])}`);
        }

        ctx.delete(value);

        if (valStrings.length === 0) {
          return "{}";
        }
        return `{ ${valStrings.join(", ")} }`;
      }
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
      // tslint:disable-next-line:no-any
      out.push(stringify(new Set<any>(), a));
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
