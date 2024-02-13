// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Command line arguments parser based on
 * [minimist](https://github.com/minimistjs/minimist).
 *
 * This module is browser compatible.
 *
 * @example
 * ```ts
 * import { parseArgs } from "https://deno.land/std@$STD_VERSION/cli/parse_args.ts";
 *
 * console.dir(parseArgs(Deno.args));
 * ```
 *
 * @module
 */
import { assert } from "../assert/assert.ts";

/** Combines recursively all intersection types and returns a new single type. */
type Id<TRecord> = TRecord extends Record<string, unknown>
  ? TRecord extends infer InferredRecord
    ? { [Key in keyof InferredRecord]: Id<InferredRecord[Key]> }
  : never
  : TRecord;

/** Converts a union type `A | B | C` into an intersection type `A & B & C`. */
type UnionToIntersection<TValue> =
  (TValue extends unknown ? (args: TValue) => unknown : never) extends
    (args: infer R) => unknown ? R extends Record<string, unknown> ? R : never
    : never;

type BooleanType = boolean | string | undefined;
type StringType = string | undefined;
type ArgType = StringType | BooleanType;

type Collectable = string | undefined;
type Negatable = string | undefined;

type UseTypes<
  TBooleans extends BooleanType,
  TStrings extends StringType,
  TCollectable extends Collectable,
> = undefined extends (
  & (false extends TBooleans ? undefined : TBooleans)
  & TCollectable
  & TStrings
) ? false
  : true;

/**
 * Creates a record with all available flags with the corresponding type and
 * default type.
 */
type Values<
  TBooleans extends BooleanType,
  TStrings extends StringType,
  TCollectable extends Collectable,
  TNegatable extends Negatable,
  TDefault extends Record<string, unknown> | undefined,
  TAliases extends Aliases | undefined,
> = UseTypes<TBooleans, TStrings, TCollectable> extends true ?
    & Record<string, unknown>
    & AddAliases<
      SpreadDefaults<
        & CollectValues<TStrings, string, TCollectable, TNegatable>
        & RecursiveRequired<CollectValues<TBooleans, boolean, TCollectable>>
        & CollectUnknownValues<
          TBooleans,
          TStrings,
          TCollectable,
          TNegatable
        >,
        DedotRecord<TDefault>
      >,
      TAliases
    >
  // deno-lint-ignore no-explicit-any
  : Record<string, any>;

type Aliases<TArgNames = string, TAliasNames extends string = string> = Partial<
  Record<Extract<TArgNames, string>, TAliasNames | ReadonlyArray<TAliasNames>>
>;

type AddAliases<
  TArgs,
  TAliases extends Aliases | undefined,
> = {
  [TArgName in keyof TArgs as AliasNames<TArgName, TAliases>]: TArgs[TArgName];
};

type AliasNames<
  TArgName,
  TAliases extends Aliases | undefined,
> = TArgName extends keyof TAliases
  ? string extends TAliases[TArgName] ? TArgName
  : TAliases[TArgName] extends string ? TArgName | TAliases[TArgName]
  : TAliases[TArgName] extends Array<string>
    ? TArgName | TAliases[TArgName][number]
  : TArgName
  : TArgName;

/**
 * Spreads all default values of Record `TDefaults` into Record `TArgs`
 * and makes default values required.
 *
 * **Example:**
 * `SpreadValues<{ foo?: boolean, bar?: number }, { foo: number }>`
 *
 * **Result:** `{ foo: boolean | number, bar?: number }`
 */
type SpreadDefaults<TArgs, TDefaults> = TDefaults extends undefined ? TArgs
  : TArgs extends Record<string, unknown> ?
      & Omit<TArgs, keyof TDefaults>
      & {
        [Default in keyof TDefaults]: Default extends keyof TArgs
          ? (TArgs[Default] & TDefaults[Default] | TDefaults[Default]) extends
            Record<string, unknown>
            ? NonNullable<SpreadDefaults<TArgs[Default], TDefaults[Default]>>
          : TDefaults[Default] | NonNullable<TArgs[Default]>
          : unknown;
      }
  : never;

/**
 * Defines the Record for the `default` option to add
 * auto-suggestion support for IDE's.
 */
type Defaults<TBooleans extends BooleanType, TStrings extends StringType> = Id<
  UnionToIntersection<
    & Record<string, unknown>
    // Dedotted auto suggestions: { foo: { bar: unknown } }
    & MapTypes<TStrings, unknown>
    & MapTypes<TBooleans, unknown>
    // Flat auto suggestions: { "foo.bar": unknown }
    & MapDefaults<TBooleans>
    & MapDefaults<TStrings>
  >
>;

type MapDefaults<TArgNames extends ArgType> = Partial<
  Record<TArgNames extends string ? TArgNames : string, unknown>
>;

type RecursiveRequired<TRecord> = TRecord extends Record<string, unknown> ? {
    [Key in keyof TRecord]-?: RecursiveRequired<TRecord[Key]>;
  }
  : TRecord;

/** Same as `MapTypes` but also supports collectable options. */
type CollectValues<
  TArgNames extends ArgType,
  TType,
  TCollectable extends Collectable,
  TNegatable extends Negatable = undefined,
> = UnionToIntersection<
  Extract<TArgNames, TCollectable> extends string ?
      & (Exclude<TArgNames, TCollectable> extends never ? Record<never, never>
        : MapTypes<Exclude<TArgNames, TCollectable>, TType, TNegatable>)
      & (Extract<TArgNames, TCollectable> extends never ? Record<never, never>
        : RecursiveRequired<
          MapTypes<Extract<TArgNames, TCollectable>, Array<TType>, TNegatable>
        >)
    : MapTypes<TArgNames, TType, TNegatable>
>;

/** Same as `Record` but also supports dotted and negatable options. */
type MapTypes<
  TArgNames extends ArgType,
  TType,
  TNegatable extends Negatable = undefined,
> = undefined extends TArgNames ? Record<never, never>
  : TArgNames extends `${infer Name}.${infer Rest}` ? {
      [Key in Name]?: MapTypes<
        Rest,
        TType,
        TNegatable extends `${Name}.${infer Negate}` ? Negate : undefined
      >;
    }
  : TArgNames extends string ? Partial<
      Record<TArgNames, TNegatable extends TArgNames ? TType | false : TType>
    >
  : Record<never, never>;

type CollectUnknownValues<
  TBooleans extends BooleanType,
  TStrings extends StringType,
  TCollectable extends Collectable,
  TNegatable extends Negatable,
> = UnionToIntersection<
  TCollectable extends TBooleans & TStrings ? Record<never, never>
    : DedotRecord<
      // Unknown collectable & non-negatable args.
      & Record<
        Exclude<
          Extract<Exclude<TCollectable, TNegatable>, string>,
          Extract<TStrings | TBooleans, string>
        >,
        Array<unknown>
      >
      // Unknown collectable & negatable args.
      & Record<
        Exclude<
          Extract<Extract<TCollectable, TNegatable>, string>,
          Extract<TStrings | TBooleans, string>
        >,
        Array<unknown> | false
      >
    >
>;

/** Converts `{ "foo.bar.baz": unknown }` into `{ foo: { bar: { baz: unknown } } }`. */
type DedotRecord<TRecord> = Record<string, unknown> extends TRecord ? TRecord
  : TRecord extends Record<string, unknown> ? UnionToIntersection<
      ValueOf<
        {
          [Key in keyof TRecord]: Key extends string ? Dedot<Key, TRecord[Key]>
            : never;
        }
      >
    >
  : TRecord;

type Dedot<TKey extends string, TValue> = TKey extends
  `${infer Name}.${infer Rest}` ? { [Key in Name]: Dedot<Rest, TValue> }
  : { [Key in TKey]: TValue };

type ValueOf<TValue> = TValue[keyof TValue];

/** The value returned from `parseArgs`. */
export type Args<
  // deno-lint-ignore no-explicit-any
  TArgs extends Record<string, unknown> = Record<string, any>,
  TDoubleDash extends boolean | undefined = undefined,
> = Id<
  & TArgs
  & {
    /** Contains all the arguments that didn't have an option associated with
     * them. */
    _: Array<string | number>;
  }
  & (boolean extends TDoubleDash ? DoubleDash
    : true extends TDoubleDash ? Required<DoubleDash>
    : Record<never, never>)
>;

type DoubleDash = {
  /** Contains all the arguments that appear after the double dash: "--". */
  "--"?: Array<string>;
};

/** The options for the `parseArgs` call. */
export interface ParseOptions<
  TBooleans extends BooleanType = BooleanType,
  TStrings extends StringType = StringType,
  TCollectable extends Collectable = Collectable,
  TNegatable extends Negatable = Negatable,
  TDefault extends Record<string, unknown> | undefined =
    | Record<string, unknown>
    | undefined,
  TAliases extends Aliases | undefined = Aliases | undefined,
  TDoubleDash extends boolean | undefined = boolean | undefined,
> {
  /**
   * When `true`, populate the result `_` with everything before the `--` and
   * the result `['--']` with everything after the `--`.
   *
   * @default {false}
   *
   *  @example
   * ```ts
   * // $ deno run example.ts -- a arg1
   * import { parseArgs } from "https://deno.land/std@$STD_VERSION/cli/parse_args.ts";
   * console.dir(parseArgs(Deno.args, { "--": false }));
   * // output: { _: [ "a", "arg1" ] }
   * console.dir(parseArgs(Deno.args, { "--": true }));
   * // output: { _: [], --: [ "a", "arg1" ] }
   * ```
   */
  "--"?: TDoubleDash;

  /**
   * An object mapping string names to strings or arrays of string argument
   * names to use as aliases.
   */
  alias?: TAliases;

  /**
   * A boolean, string or array of strings to always treat as booleans. If
   * `true` will treat all double hyphenated arguments without equal signs as
   * `boolean` (e.g. affects `--foo`, not `-f` or `--foo=bar`).
   *  All `boolean` arguments will be set to `false` by default.
   */
  boolean?: TBooleans | ReadonlyArray<Extract<TBooleans, string>>;

  /** An object mapping string argument names to default values. */
  default?: TDefault & Defaults<TBooleans, TStrings>;

  /**
   * When `true`, populate the result `_` with everything after the first
   * non-option.
   */
  stopEarly?: boolean;

  /** A string or array of strings argument names to always treat as strings. */
  string?: TStrings | ReadonlyArray<Extract<TStrings, string>>;

  /**
   * A string or array of strings argument names to always treat as arrays.
   * Collectable options can be used multiple times. All values will be
   * collected into one array. If a non-collectable option is used multiple
   * times, the last value is used.
   * All Collectable arguments will be set to `[]` by default.
   */
  collect?: TCollectable | ReadonlyArray<Extract<TCollectable, string>>;

  /**
   * A string or array of strings argument names which can be negated
   * by prefixing them with `--no-`, like `--no-config`.
   */
  negatable?: TNegatable | ReadonlyArray<Extract<TNegatable, string>>;

  /**
   * A function which is invoked with a command line parameter not defined in
   * the `options` configuration object. If the function returns `false`, the
   * unknown option is not added to `parsedArgs`.
   */
  unknown?: (arg: string, key?: string, value?: unknown) => unknown;
}

interface Flags {
  bools: Record<string, boolean>;
  strings: Record<string, boolean>;
  collect: Record<string, boolean>;
  negatable: Record<string, boolean>;
  unknownFn: (arg: string, key?: string, value?: unknown) => unknown;
  allBools: boolean;
}

interface NestedMapping {
  [key: string]: NestedMapping | unknown;
}

const { hasOwn } = Object;

function get<TValue>(
  obj: Record<string, TValue>,
  key: string,
): TValue | undefined {
  if (hasOwn(obj, key)) {
    return obj[key];
  }
}

function getForce<TValue>(obj: Record<string, TValue>, key: string): TValue {
  const v = get(obj, key);
  assert(v !== undefined);
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
  return hasOwn(o, key);
}

/** Take a set of command line arguments, optionally with a set of options, and
 * return an object representing the flags found in the passed arguments.
 *
 * By default, any arguments starting with `-` or `--` are considered boolean
 * flags. If the argument name is followed by an equal sign (`=`) it is
 * considered a key-value pair. Any arguments which could not be parsed are
 * available in the `_` property of the returned object.
 *
 * By default, the flags module tries to determine the type of all arguments
 * automatically and the return type of the `parseArgs` method will have an index
 * signature with `any` as value (`{ [x: string]: any }`).
 *
 * If the `string`, `boolean` or `collect` option is set, the return value of
 * the `parseArgs` method will be fully typed and the index signature of the return
 * type will change to `{ [x: string]: unknown }`.
 *
 * Any arguments after `'--'` will not be parsed and will end up in `parsedArgs._`.
 *
 * Numeric-looking arguments will be returned as numbers unless `options.string`
 * or `options.boolean` is set for that argument name.
 *
 * @example
 * ```ts
 * import { parseArgs } from "https://deno.land/std@$STD_VERSION/cli/parse_args.ts";
 * const parsedArgs = parseArgs(Deno.args);
 * ```
 *
 * @example
 * ```ts
 * import { parseArgs } from "https://deno.land/std@$STD_VERSION/cli/parse_args.ts";
 * const parsedArgs = parseArgs(["--foo", "--bar=baz", "./quux.txt"]);
 * // parsedArgs: { foo: true, bar: "baz", _: ["./quux.txt"] }
 * ```
 */
export function parseArgs<
  TArgs extends Values<
    TBooleans,
    TStrings,
    TCollectable,
    TNegatable,
    TDefaults,
    TAliases
  >,
  TDoubleDash extends boolean | undefined = undefined,
  TBooleans extends BooleanType = undefined,
  TStrings extends StringType = undefined,
  TCollectable extends Collectable = undefined,
  TNegatable extends Negatable = undefined,
  TDefaults extends Record<string, unknown> | undefined = undefined,
  TAliases extends Aliases<TAliasArgNames, TAliasNames> | undefined = undefined,
  TAliasArgNames extends string = string,
  TAliasNames extends string = string,
>(
  args: string[],
  {
    "--": doubleDash = false,
    alias = {} as NonNullable<TAliases>,
    boolean = false,
    default: defaults = {} as TDefaults & Defaults<TBooleans, TStrings>,
    stopEarly = false,
    string = [],
    collect = [],
    negatable = [],
    unknown = (i: string): unknown => i,
  }: ParseOptions<
    TBooleans,
    TStrings,
    TCollectable,
    TNegatable,
    TDefaults,
    TAliases,
    TDoubleDash
  > = {},
): Args<TArgs, TDoubleDash> {
  const aliases: Record<string, string[]> = {};
  const flags: Flags = {
    bools: {},
    strings: {},
    unknownFn: unknown,
    allBools: false,
    collect: {},
    negatable: {},
  };

  if (alias !== undefined) {
    for (const key in alias) {
      const val = getForce(alias, key);
      if (typeof val === "string") {
        aliases[key] = [val];
      } else {
        aliases[key] = val as Array<string>;
      }
      for (const alias of getForce(aliases, key)) {
        aliases[alias] = [key].concat(aliases[key].filter((y) => alias !== y));
      }
    }
  }

  if (boolean !== undefined) {
    if (typeof boolean === "boolean") {
      flags.allBools = !!boolean;
    } else {
      const booleanArgs: ReadonlyArray<string> = typeof boolean === "string"
        ? [boolean]
        : boolean;

      for (const key of booleanArgs.filter(Boolean)) {
        flags.bools[key] = true;
        const alias = get(aliases, key);
        if (alias) {
          for (const al of alias) {
            flags.bools[al] = true;
          }
        }
      }
    }
  }

  if (string !== undefined) {
    const stringArgs: ReadonlyArray<string> = typeof string === "string"
      ? [string]
      : string;

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

  if (collect !== undefined) {
    const collectArgs: ReadonlyArray<string> = typeof collect === "string"
      ? [collect]
      : collect;

    for (const key of collectArgs.filter(Boolean)) {
      flags.collect[key] = true;
      const alias = get(aliases, key);
      if (alias) {
        for (const al of alias) {
          flags.collect[al] = true;
        }
      }
    }
  }

  if (negatable !== undefined) {
    const negatableArgs: ReadonlyArray<string> = typeof negatable === "string"
      ? [negatable]
      : negatable;

    for (const key of negatableArgs.filter(Boolean)) {
      flags.negatable[key] = true;
      const alias = get(aliases, key);
      if (alias) {
        for (const al of alias) {
          flags.negatable[al] = true;
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

  function setKey(
    obj: NestedMapping,
    name: string,
    value: unknown,
    collect = true,
  ) {
    let o = obj;
    const keys = name.split(".");
    keys.slice(0, -1).forEach(function (key) {
      if (get(o, key) === undefined) {
        o[key] = {};
      }
      o = get(o, key) as NestedMapping;
    });

    const key = keys[keys.length - 1];
    const collectable = collect && !!get(flags.collect, name);

    if (!collectable) {
      o[key] = value;
    } else if (get(o, key) === undefined) {
      o[key] = [value];
    } else if (Array.isArray(get(o, key))) {
      (o[key] as unknown[]).push(value);
    } else {
      o[key] = [get(o, key), value];
    }
  }

  function setArg(
    key: string,
    val: unknown,
    arg: string | undefined = undefined,
    collect?: boolean,
  ) {
    if (arg && flags.unknownFn && !argDefined(key, arg)) {
      if (flags.unknownFn(arg, key, val) === false) return;
    }

    const value = !get(flags.strings, key) && isNumber(val) ? Number(val) : val;
    setKey(argv, key, value, collect);

    const alias = get(aliases, key);
    if (alias) {
      for (const x of alias) {
        setKey(argv, x, value, collect);
      }
    }
  }

  function aliasIsBoolean(key: string): boolean {
    return getForce(aliases, key).some(
      (x) => typeof get(flags.bools, x) === "boolean",
    );
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
      assert(m !== null);
      const [, key, value] = m;

      if (flags.bools[key]) {
        const booleanValue = value !== "false";
        setArg(key, booleanValue, arg);
      } else {
        setArg(key, value, arg);
      }
    } else if (
      /^--no-.+/.test(arg) && get(flags.negatable, arg.replace(/^--no-/, ""))
    ) {
      const m = arg.match(/^--no-(.+)/);
      assert(m !== null);
      setArg(m[1], false, arg, false);
    } else if (/^--.+/.test(arg)) {
      const m = arg.match(/^--(.+)/);
      assert(m !== null);
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
          setArg(letters[j], next.split(/=(.+)/)[1], arg);
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

  for (const [key, value] of Object.entries(defaults)) {
    if (!hasKey(argv, key.split("."))) {
      setKey(argv, key, value, false);

      if (aliases[key]) {
        for (const x of aliases[key]) {
          setKey(argv, x, value, false);
        }
      }
    }
  }

  for (const key of Object.keys(flags.bools)) {
    if (!hasKey(argv, key.split("."))) {
      const value = get(flags.collect, key) ? [] : false;
      setKey(
        argv,
        key,
        value,
        false,
      );
    }
  }

  for (const key of Object.keys(flags.strings)) {
    if (!hasKey(argv, key.split(".")) && get(flags.collect, key)) {
      setKey(
        argv,
        key,
        [],
        false,
      );
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

  return argv as Args<TArgs, TDoubleDash>;
}
