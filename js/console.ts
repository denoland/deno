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
  if (instance) {
    const proto = Object.getPrototypeOf(instance);
    if (proto && proto.constructor) {
      return proto.constructor.name; // could be "Object" or "Array"
    }
  }
  return "";
}

function createFunctionString(value: Function, ctx: ConsoleContext): string {
  // Might be Function/AsyncFunction/GeneratorFunction
  const cstrName = Object.getPrototypeOf(value).constructor.name;
  if (value.name && value.name !== "anonymous") {
    // from MDN spec
    return `[${cstrName}: ${value.name}]`;
  }
  return `[${cstrName}]`;
}

function createArrayString(
  // tslint:disable-next-line:no-any
  value: any[],
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const entries: string[] = [];
  for (const el of value) {
    entries.push(stringifyWithQuotes(ctx, el, level + 1, maxLevel));
  }
  ctx.delete(value);
  if (entries.length === 0) {
    return "[]";
  }
  return `[ ${entries.join(", ")} ]`;
}

function createObjectString(
  // tslint:disable-next-line:no-any
  value: any,
  ctx: ConsoleContext,
  level: number,
  maxLevel: number
): string {
  const entries: string[] = [];
  let baseString = "";

  const className = getClassInstanceName(value);
  let shouldShowClassName = false;
  if (className && className !== "Object" && className !== "anonymous") {
    shouldShowClassName = true;
  }

  for (const key of Object.keys(value)) {
    entries.push(
      `${key}: ${stringifyWithQuotes(ctx, value[key], level + 1, maxLevel)}`
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
      return createFunctionString(value as Function, ctx);
    case "object":
      if (value === null) {
        return "null";
      }

      if (ctx.has(value)) {
        return "[Circular]";
      }

      if (level >= maxLevel) {
        return `[object]`;
      }

      ctx.add(value);

      if (value instanceof Error) {
        return value.stack! || "";
      } else if (Array.isArray(value)) {
        // tslint:disable-next-line:no-any
        return createArrayString(value as any[], ctx, level, maxLevel);
      } else {
        return createObjectString(value, ctx, level, maxLevel);
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

// @internal
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
        // use default maximum depth for null or undefined argument
        stringify(
          // tslint:disable-next-line:no-any
          new Set<any>(),
          a,
          0,
          // tslint:disable-next-line:triple-equals
          options.depth != undefined ? options.depth : DEFAULT_MAX_DEPTH
        )
      );
    }
  }
  return out.join(" ");
}

type PrintFunc = (x: string, isErr?: boolean) => void;

export class Console {
  // @internal
  constructor(private printFunc: PrintFunc) {}

  /** Writes the arguments to stdout */
  // tslint:disable-next-line:no-any
  log = (...args: any[]): void => {
    this.printFunc(stringifyArgs(args));
  };

  /** Writes the arguments to stdout */
  debug = this.log;
  /** Writes the arguments to stdout */
  info = this.log;

  /** Writes the properties of the supplied `obj` to stdout */
  // tslint:disable-next-line:no-any
  dir = (obj: any, options: ConsoleOptions = {}) => {
    this.printFunc(stringifyArgs([obj], options));
  };

  /** Writes the arguments to stdout */
  // tslint:disable-next-line:no-any
  warn = (...args: any[]): void => {
    this.printFunc(stringifyArgs(args), true);
  };

  /** Writes the arguments to stdout */
  error = this.warn;

  /** Writes an error message to stdout if the assertion is `false`. If the
   * assertion is `true`, nothing happens.
   */
  // tslint:disable-next-line:no-any
  assert = (condition: boolean, ...args: any[]): void => {
    if (!condition) {
      throw new Error(`Assertion failed: ${stringifyArgs(args)}`);
    }
  };
}
