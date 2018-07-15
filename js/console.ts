// tslint:disable-next-line:no-any
type ConsoleContext = Set<any>;

// tslint:disable-next-line:no-any
function getClassInstanceName(instance: any): string {
  if (typeof instance !== "object") {
    return "";
  }
  if (instance && instance.__proto__ && instance.__proto__.constructor) {
    return instance.__proto__.constructor.name; // could be "Object" or "Array"
  }
  return "";
}

// tslint:disable-next-line:no-any
function stringify(ctx: ConsoleContext, value: any): string {
  switch (typeof value) {
    case "string":
      return value;
    case "number":
    case "boolean":
    case "undefined":
    case "symbol":
      return String(value);
    case "function":
      if (value.name && value.name !== "anonymous") {
        // from MDN spec
        return `[Function: ${value.name}]`;
      }
      return "[Function]";
    case "object":
      if (value === null) {
        return "null";
      }

      if (ctx.has(value)) {
        return "[Circular]";
      }

      ctx.add(value);
      const entries: string[] = [];

      if (Array.isArray(value)) {
        for (const el of value) {
          entries.push(stringify(ctx, el));
        }

        ctx.delete(value);

        if (entries.length === 0) {
          return "[]";
        }
        return `[ ${entries.join(", ")} ]`;
      } else {
        let baseString = "";

        const className = getClassInstanceName(value);
        let shouldShowClassName = false;
        if (className && className !== "Object" && className !== "anonymous") {
          shouldShowClassName = true;
        }

        for (const key of Object.keys(value)) {
          entries.push(`${key}: ${stringify(ctx, value[key])}`);
        }

        ctx.delete(value);

        if (entries.length === 0) {
          baseString = "{}";
        } else {
          baseString = `{ ${entries.join(", ")} }`;
        }

        if (shouldShowClassName) {
          baseString = `${className} ${baseString}`;
        }

        return baseString;
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

export class Console {
  // tslint:disable-next-line:no-any
  log(...args: any[]): void {
    deno.print(stringifyArgs(args));
  }

  debug = this.log;
  info = this.log;

  // tslint:disable-next-line:no-any
  warn(...args: any[]): void {
    deno.print(`ERROR: ${stringifyArgs(args)}`);
  }

  error = this.warn;

  // tslint:disable-next-line:no-any
  assert(condition: boolean, ...args: any[]): void {
    if (!condition) {
      throw new Error(`Assertion failed: ${stringifyArgs(args)}`);
    }
  }
}
