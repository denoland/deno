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
      // Might be Function/AsyncFunction/GeneratorFunction
      const cstrName = value.__proto__.constructor.name;
      if (value.name && value.name !== "anonymous") {
        // from MDN spec
        return `[${cstrName}: ${value.name}]`;
      }
      return `[${cstrName}]`;
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
          entries.push(stringifyWithQuotes(ctx, el));
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
          entries.push(`${key}: ${stringifyWithQuotes(ctx, value[key])}`);
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

// Print strings when they are inside of arrays or objects with quotes
// tslint:disable-next-line:no-any
function stringifyWithQuotes(ctx: ConsoleContext, value: any): string {
  switch (typeof value) {
    case "string":
      return `"${value}"`;
    default:
      return stringify(ctx, value);
  }
}

// tslint:disable-next-line:no-any
export function stringifyArgs(args: any[]): string {
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

type PrintFunc = (x: string, isErr?: boolean) => void;

export class Console {
  constructor(private printFunc: PrintFunc) {}

  // tslint:disable-next-line:no-any
  log(...args: any[]): void {
    this.printFunc(stringifyArgs(args));
  }

  debug = this.log;
  info = this.log;

  // tslint:disable-next-line:no-any
  warn(...args: any[]): void {
    this.printFunc(stringifyArgs(args), true);
  }

  error = this.warn;

  // tslint:disable-next-line:no-any
  assert(condition: boolean, ...args: any[]): void {
    if (!condition) {
      throw new Error(`Assertion failed: ${stringifyArgs(args)}`);
    }
  }
}
