// tslint:disable-next-line:no-any
type ConsoleContext = Set<any>;
type ConsoleOptions = Partial<{
  showHidden: boolean;
  depth: number;
  colors: boolean;
}>;

// Default depth of logging nested objects
const DEFAULT_MAX_DEPTH = 2;

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

function stringify(
  ctx: ConsoleContext,
  // tslint:disable-next-line:no-any
  value: any,
  level: number,
  maxLevel: number
): string {
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

      if (level > maxLevel) {
        return `[object]`;
      }

      ctx.add(value);
      const entries: string[] = [];

      if (Array.isArray(value)) {
        for (const el of value) {
          entries.push(stringifyWithQuotes(ctx, el, level + 1, maxLevel));
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
          entries.push(
            `${key}: ${stringifyWithQuotes(
              ctx,
              value[key],
              level + 1,
              maxLevel
            )}`
          );
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
function stringifyWithQuotes(
  ctx: ConsoleContext,
  // tslint:disable-next-line:no-any
  value: any,
  level: number,
  maxLevel: number
): string {
  switch (typeof value) {
    case "string":
      return `"${value}"`;
    default:
      return stringify(ctx, value, level, maxLevel);
  }
}

export function stringifyArgs(
  // tslint:disable-next-line:no-any
  args: any[],
  options: ConsoleOptions = {}
): string {
  const out: string[] = [];
  for (const a of args) {
    if (typeof a === "string") {
      out.push(a);
    } else {
      out.push(
        // tslint:disable-next-line:no-any
        stringify(new Set<any>(), a, 0, options.depth || DEFAULT_MAX_DEPTH)
      );
    }
  }
  return out.join(" ");
}

type PrintFunc = (x: string) => void;

export class Console {
  constructor(private printFunc: PrintFunc) {}

  // tslint:disable-next-line:no-any
  log(...args: any[]): void {
    this.printFunc(stringifyArgs(args));
  }

  debug = this.log;
  info = this.log;

  // tslint:disable-next-line:no-any
  dir(obj: any, options: ConsoleOptions = {}) {
    this.printFunc(
      stringifyArgs([obj], { depth: options.depth || DEFAULT_MAX_DEPTH })
    );
  }

  // tslint:disable-next-line:no-any
  warn(...args: any[]): void {
    // TODO Log to stderr.
    this.printFunc(stringifyArgs(args));
  }

  error = this.warn;

  // tslint:disable-next-line:no-any
  assert(condition: boolean, ...args: any[]): void {
    if (!condition) {
      throw new Error(`Assertion failed: ${stringifyArgs(args)}`);
    }
  }
}
