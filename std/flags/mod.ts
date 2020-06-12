// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assert } from "../_util/assert.ts";

export interface Args {
  /** Contains all the arguments that didn't have an option associated with
   * them. */
  _: Array<string | number>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [key: string]: any;
}

export interface ArgParsingOptions {
  /** When `true`, populate the result `_` with everything before the `--` and
   * the result `['--']` with everything after the `--`. Here's an example:
   *
   *      // $ deno run example.ts -- a arg1
   *      const { args } = Deno;
   *      import { parse } from "https://deno.land/std/flags/mod.ts";
   *      console.dir(parse(args, { "--": false }));
   *      // output: { _: [ "a", "arg1" ] }
   *      console.dir(parse(args, { "--": true }));
   *      // output: { _: [], --: [ "a", "arg1" ] }
   *
   * Defaults to `false`.
   */
  "--"?: boolean;

  /** An object mapping string names to strings or arrays of string argument
   * names to use as aliases */
  alias?: Record<string, string | string[]>;

  /** A boolean, string or array of strings to always treat as booleans. If
   * `true` will treat all double hyphenated arguments without equal signs as
   * `boolean` (e.g. affects `--foo`, not `-f` or `--foo=bar`) */
  boolean?: boolean | string | string[];

  /** An object mapping string argument names to default values. */
  default?: Record<string, unknown>;

  /** When `true`, populate the result `_` with everything after the first
   * non-option. */
  stopEarly?: boolean;

  /** A string or array of strings argument names to always treat as strings. */
  string?: string | string[];

  /** A function which is invoked with a command line parameter not defined in
   * the `options` configuration object. If the function returns `false`, the
   * unknown option is not added to `parsedArgs`. */
  unknown?: (arg: string, key?: string, value?: unknown) => unknown;
}

interface Flags {
  bools: Record<string, boolean>;
  strings: Record<string, boolean>;
  unknownFn: (arg: string, key?: string, value?: unknown) => unknown;
  allBools: boolean;
}

interface NestedMapping {
  [key: string]: NestedMapping | unknown;
}

function get<T>(obj: Record<string, T>, key: string): T | undefined {
  if (Object.prototype.hasOwnProperty.call(obj, key)) {
    return obj[key];
  }
}

function getForce<T>(obj: Record<string, T>, key: string): T {
  const v = get(obj, key);
  assert(v != null);
  return v;
}

function isNumber(x: unknown): boolean {
  if (typeof x === "number") return true;
  if (/^0x[0-9a-f]+$/i.test(String(x))) return true;
  return /^[-+]?(?:\d+(?:\.\d*)?|\.\d+)(e[-+]?\d+)?$/.test(String(x));
}

function hasKey(obj: NestedMapping, keys: string[]): boolean {
  let o = obj;
  keys.slice(0, -1).forEach((key) => {
    o = (get(o, key) ?? {}) as NestedMapping;
  });

  const key = keys[keys.length - 1];
  return key in o;
}

/** Take a set of command line arguments, with an optional set of options, and
 * return an object representation of those argument.
 *
 *      const parsedArgs = parse(Deno.args);
 */
export function parse(
  args: string[],
  {
    "--": doubleDash = false,
    alias = {},
    boolean = false,
    default: defaults = {},
    stopEarly = false,
    string = [],
    unknown = (i: string): unknown => i,
  }: ArgParsingOptions = {}
): Args {
  const flags: Flags = {
    bools: {},
    strings: {},
    unknownFn: unknown,
    allBools: false,
  };

  if (boolean !== undefined) {
    if (typeof boolean === "boolean") {
      flags.allBools = !!boolean;
    } else {
      const booleanArgs = typeof boolean === "string" ? [boolean] : boolean;

      for (const key of booleanArgs.filter(Boolean)) {
        flags.bools[key] = true;
      }
    }
  }

  const aliases: Record<string, string[]> = {};
  if (alias !== undefined) {
    for (const key in alias) {
      const val = getForce(alias, key);
      if (typeof val === "string") {
        aliases[key] = [val];
      } else {
        aliases[key] = val;
      }
      for (const alias of getForce(aliases, key)) {
        aliases[alias] = [key].concat(aliases[key].filter((y) => alias !== y));
      }
    }
  }

  if (string !== undefined) {
    const stringArgs = typeof string === "string" ? [string] : string;

    for (const key of stringArgs.filter(Boolean)) {
      flags.strings[key] = true;
      const alias = get(aliases, key);
      if (alias) {
        for (const al of alias) {
          flags.strings[al] = true;
        }
      }
    }
  }

  const argv: Args = { _: [] };

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
    keys.slice(0, -1).forEach(function (key): void {
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
      if (flags.unknownFn(arg, key, val) === false) return;
    }

    const value = !get(flags.strings, key) && isNumber(val) ? Number(val) : val;
    setKey(argv, key.split("."), value);

    const alias = get(aliases, key);
    if (alias) {
      for (const x of alias) {
        setKey(argv, x.split("."), value);
      }
    }
  }

  function aliasIsBoolean(key: string): boolean {
    return getForce(aliases, key).some(
      (x) => typeof get(flags.bools, x) === "boolean"
    );
  }

  for (const key of Object.keys(flags.bools)) {
    setArg(key, defaults[key] === undefined ? false : defaults[key]);
  }

  let notFlags: string[] = [];

  // all args after "--" are not parsed
  if (args.includes("--")) {
    notFlags = args.slice(args.indexOf("--") + 1);
    args = args.slice(0, args.indexOf("--"));
  }

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    if (/^--.+=/.test(arg)) {
      const m = arg.match(/^--([^=]+)=(.*)$/s);
      assert(m != null);
      const [, key, value] = m;

      if (flags.bools[key]) {
        const booleanValue = value !== "false";
        setArg(key, booleanValue, arg);
      } else {
        setArg(key, value, arg);
      }
    } else if (/^--no-.+/.test(arg)) {
      const m = arg.match(/^--no-(.+)/);
      assert(m != null);
      setArg(m[1], false, arg);
    } else if (/^--.+/.test(arg)) {
      const m = arg.match(/^--(.+)/);
      assert(m != null);
      const [, key] = m;
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

      const [key] = arg.slice(-1);
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
        argv._.push(flags.strings["_"] ?? !isNumber(arg) ? arg : Number(arg));
      }
      if (stopEarly) {
        argv._.push(...args.slice(i + 1));
        break;
      }
    }
  }

  for (const key of Object.keys(defaults)) {
    if (!hasKey(argv, key.split("."))) {
      setKey(argv, key.split("."), defaults[key]);

      if (aliases[key]) {
        for (const x of aliases[key]) {
          setKey(argv, x.split("."), defaults[key]);
        }
      }
    }
  }

  if (doubleDash) {
    argv["--"] = [];
    for (const key of notFlags) {
      argv["--"].push(key);
    }
  } else {
    for (const key of notFlags) {
      argv._.push(key);
    }
  }

  return argv;
}
