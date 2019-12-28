// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
export interface ArgParsingOptions {
  unknown?: (i: unknown) => unknown;
  boolean?: boolean | string | string[];
  alias?: { [key: string]: string | string[] };
  string?: string | string[];
  default?: { [key: string]: unknown };
  "--"?: boolean;
  stopEarly?: boolean;
}

const DEFAULT_OPTIONS = {
  unknown: (i: unknown): unknown => i,
  boolean: false,
  alias: {},
  string: [],
  default: {},
  "--": false,
  stopEarly: false
};

interface Flags {
  bools: { [key: string]: boolean };
  strings: { [key: string]: boolean };
  unknownFn: (i: unknown) => unknown;
  allBools: boolean;
}

interface NestedMapping {
  [key: string]: NestedMapping | unknown;
}

function get<T>(obj: { [s: string]: T }, key: string): T | undefined {
  if (Object.prototype.hasOwnProperty.call(obj, key)) {
    return obj[key];
  }
}

function isNumber(x: unknown): boolean {
  if (typeof x === "number") return true;
  if (/^0x[0-9a-f]+$/i.test(String(x))) return true;
  return /^[-+]?(?:\d+(?:\.\d*)?|\.\d+)(e[-+]?\d+)?$/.test(String(x));
}

function hasKey(obj: NestedMapping, keys: string[]): boolean {
  let o = obj;
  keys.slice(0, -1).forEach(function(key: string): void {
    o = (get(o, key) || {}) as NestedMapping;
  });

  const key = keys[keys.length - 1];
  return key in o;
}

export function parse(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  args: any[],
  initialOptions?: ArgParsingOptions
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
): { [key: string]: any } {
  const options: ArgParsingOptions = {
    ...DEFAULT_OPTIONS,
    ...(initialOptions || {})
  };

  const flags: Flags = {
    bools: {},
    strings: {},
    unknownFn: options.unknown!,
    allBools: false
  };

  if (options.boolean !== undefined) {
    if (typeof options.boolean === "boolean") {
      flags.allBools = !!options.boolean;
    } else {
      const booleanArgs: string[] =
        typeof options.boolean === "string"
          ? [options.boolean]
          : options.boolean;

      booleanArgs.filter(Boolean).forEach((key: string): void => {
        flags.bools[key] = true;
      });
    }
  }

  const aliases: { [key: string]: string[] } = {};
  if (options.alias !== undefined) {
    for (const key in options.alias) {
      const val = get(options.alias, key)!;

      if (typeof val === "string") {
        aliases[key] = [val];
      } else {
        aliases[key] = val;
      }

      for (const alias of get(aliases, key)!) {
        aliases[alias] = [key].concat(
          aliases[key].filter((y: string): boolean => alias !== y)
        );
      }
    }
  }

  if (options.string !== undefined) {
    const stringArgs =
      typeof options.string === "string" ? [options.string] : options.string;

    stringArgs.filter(Boolean).forEach(function(key): void {
      flags.strings[key] = true;
      const alias = get(aliases, key);
      if (alias) {
        alias.forEach((alias: string): void => {
          flags.strings[alias] = true;
        });
      }
    });
  }

  const defaults = options.default!;

  const argv: { [key: string]: unknown[] } = { _: [] };

  function argDefined(key: string, arg: string): boolean {
    return (
      (flags.allBools && /^--[^=]+$/.test(arg)) ||
      get(flags.bools, key) ||
      !!get(flags.strings, key) ||
      !!get(aliases, key)
    );
  }

  function setKey(obj: NestedMapping, keys: string[], value: unknown): void {
    let o = obj;
    keys.slice(0, -1).forEach(function(key): void {
      if (get(o, key) === undefined) {
        o[key] = {};
      }
      o = get(o, key) as NestedMapping;
    });

    const key = keys[keys.length - 1];
    if (
      get(o, key) === undefined ||
      get(flags.bools, key) ||
      typeof get(o, key) === "boolean"
    ) {
      o[key] = value;
    } else if (Array.isArray(get(o, key))) {
      (o[key] as unknown[]).push(value);
    } else {
      o[key] = [get(o, key), value];
    }
  }

  function setArg(
    key: string,
    val: unknown,
    arg: string | undefined = undefined
  ): void {
    if (arg && flags.unknownFn && !argDefined(key, arg)) {
      if (flags.unknownFn(arg) === false) return;
    }

    const value = !get(flags.strings, key) && isNumber(val) ? Number(val) : val;
    setKey(argv, key.split("."), value);

    (get(aliases, key) || []).forEach(function(x): void {
      setKey(argv, x.split("."), value);
    });
  }

  function aliasIsBoolean(key: string): boolean {
    return get(aliases, key)!.some(function(x): boolean {
      return get(flags.bools, x)!;
    });
  }

  Object.keys(flags.bools).forEach(function(key): void {
    setArg(key, defaults[key] === undefined ? false : defaults[key]);
  });

  let notFlags: string[] = [];

  // all args after "--" are not parsed
  if (args.indexOf("--") !== -1) {
    notFlags = args.slice(args.indexOf("--") + 1);
    args = args.slice(0, args.indexOf("--"));
  }

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    if (/^--.+=/.test(arg)) {
      // Using [\s\S] instead of . because js doesn't support the
      // 'dotall' regex modifier. See:
      // http://stackoverflow.com/a/1068308/13216
      const m = arg.match(/^--([^=]+)=([\s\S]*)$/)!;
      const key = m[1];
      const value = m[2];

      if (flags.bools[key]) {
        const booleanValue = value !== "false";
        setArg(key, booleanValue, arg);
      } else {
        setArg(key, value, arg);
      }
    } else if (/^--no-.+/.test(arg)) {
      const key = arg.match(/^--no-(.+)/)![1];
      setArg(key, false, arg);
    } else if (/^--.+/.test(arg)) {
      const key = arg.match(/^--(.+)/)![1];
      const next = args[i + 1];
      if (
        next !== undefined &&
        !/^-/.test(next) &&
        !get(flags.bools, key) &&
        !flags.allBools &&
        (get(aliases, key) ? !aliasIsBoolean(key) : true)
      ) {
        setArg(key, next, arg);
        i++;
      } else if (/^(true|false)$/.test(next)) {
        setArg(key, next === "true", arg);
        i++;
      } else {
        setArg(key, get(flags.strings, key) ? "" : true, arg);
      }
    } else if (/^-[^-]+/.test(arg)) {
      const letters = arg.slice(1, -1).split("");

      let broken = false;
      for (let j = 0; j < letters.length; j++) {
        const next = arg.slice(j + 2);

        if (next === "-") {
          setArg(letters[j], next, arg);
          continue;
        }

        if (/[A-Za-z]/.test(letters[j]) && /=/.test(next)) {
          setArg(letters[j], next.split("=")[1], arg);
          broken = true;
          break;
        }

        if (
          /[A-Za-z]/.test(letters[j]) &&
          /-?\d+(\.\d*)?(e-?\d+)?$/.test(next)
        ) {
          setArg(letters[j], next, arg);
          broken = true;
          break;
        }

        if (letters[j + 1] && letters[j + 1].match(/\W/)) {
          setArg(letters[j], arg.slice(j + 2), arg);
          broken = true;
          break;
        } else {
          setArg(letters[j], get(flags.strings, letters[j]) ? "" : true, arg);
        }
      }

      const key = arg.slice(-1)[0];
      if (!broken && key !== "-") {
        if (
          args[i + 1] &&
          !/^(-|--)[^-]/.test(args[i + 1]) &&
          !get(flags.bools, key) &&
          (get(aliases, key) ? !aliasIsBoolean(key) : true)
        ) {
          setArg(key, args[i + 1], arg);
          i++;
        } else if (args[i + 1] && /^(true|false)$/.test(args[i + 1])) {
          setArg(key, args[i + 1] === "true", arg);
          i++;
        } else {
          setArg(key, get(flags.strings, key) ? "" : true, arg);
        }
      }
    } else {
      if (!flags.unknownFn || flags.unknownFn(arg) !== false) {
        argv._.push(flags.strings["_"] || !isNumber(arg) ? arg : Number(arg));
      }
      if (options.stopEarly) {
        argv._.push(...args.slice(i + 1));
        break;
      }
    }
  }

  Object.keys(defaults).forEach(function(key): void {
    if (!hasKey(argv, key.split("."))) {
      setKey(argv, key.split("."), defaults[key]);

      (aliases[key] || []).forEach(function(x): void {
        setKey(argv, x.split("."), defaults[key]);
      });
    }
  });

  if (options["--"]) {
    argv["--"] = [];
    notFlags.forEach(function(key): void {
      argv["--"].push(key);
    });
  } else {
    notFlags.forEach(function(key): void {
      argv._.push(key);
    });
  }

  return argv;
}
