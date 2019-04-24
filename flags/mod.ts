// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
export interface ArgParsingOptions {
  unknown?: Function;
  boolean?: boolean | string | string[];
  alias?: { [key: string]: string | string[] };
  string?: string | string[];
  default?: { [key: string]: any }; // eslint-disable-line @typescript-eslint/no-explicit-any
  "--"?: boolean;
  stopEarly?: boolean;
}

const DEFAULT_OPTIONS = {
  unknown: (i): unknown => i,
  boolean: false,
  alias: {},
  string: [],
  default: {},
  "--": false,
  stopEarly: false
};

function isNumber(x: unknown): boolean {
  if (typeof x === "number") return true;
  if (/^0x[0-9a-f]+$/i.test(String(x))) return true;
  return /^[-+]?(?:\d+(?:\.\d*)?|\.\d+)(e[-+]?\d+)?$/.test(String(x));
}

function hasKey(obj, keys): boolean {
  let o = obj;
  keys.slice(0, -1).forEach(function(key): void {
    o = o[key] || {};
  });

  const key = keys[keys.length - 1];
  return key in o;
}

export function parse(
  args,
  initialOptions?: ArgParsingOptions
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
): { [key: string]: any } {
  const options: ArgParsingOptions = {
    ...DEFAULT_OPTIONS,
    ...(initialOptions || {})
  };

  const flags = {
    bools: {},
    strings: {},
    unknownFn: options.unknown!,
    allBools: false
  };

  // TODO: get rid of this, providing two different options
  if (typeof options["boolean"] === "boolean" && options["boolean"]) {
    flags.allBools = true;
  } else {
    []
      .concat(options["boolean"])
      .filter(Boolean)
      .forEach(function(key): void {
        flags.bools[key] = true;
      });
  }

  const aliases = {};
  Object.keys(options.alias).forEach(function(key): void {
    aliases[key] = [].concat(options.alias[key]);
    aliases[key].forEach(function(x): void {
      aliases[x] = [key].concat(
        aliases[key].filter(function(y): boolean {
          return x !== y;
        })
      );
    });
  });

  []
    .concat(options.string)
    .filter(Boolean)
    .forEach(function(key): void {
      flags.strings[key] = true;
      if (aliases[key]) {
        flags.strings[aliases[key]] = true;
      }
    });

  const defaults = options.default!;

  const argv = { _: [] };

  function argDefined(key, arg): boolean {
    return (
      (flags.allBools && /^--[^=]+$/.test(arg)) ||
      flags.strings[key] ||
      flags.bools[key] ||
      aliases[key]
    );
  }

  function setKey(obj, keys, value): void {
    let o = obj;
    keys.slice(0, -1).forEach(function(key): void {
      if (o[key] === undefined) o[key] = {};
      o = o[key];
    });

    const key = keys[keys.length - 1];
    if (
      o[key] === undefined ||
      flags.bools[key] ||
      typeof o[key] === "boolean"
    ) {
      o[key] = value;
    } else if (Array.isArray(o[key])) {
      o[key].push(value);
    } else {
      o[key] = [o[key], value];
    }
  }

  function setArg(key, val, arg = null): void {
    if (arg && flags.unknownFn && !argDefined(key, arg)) {
      if (flags.unknownFn(arg) === false) return;
    }

    const value = !flags.strings[key] && isNumber(val) ? Number(val) : val;
    setKey(argv, key.split("."), value);

    (aliases[key] || []).forEach(function(x): void {
      setKey(argv, x.split("."), value);
    });
  }

  function aliasIsBoolean(key): boolean {
    return aliases[key].some(function(x): boolean {
      return flags.bools[x];
    });
  }

  Object.keys(flags.bools).forEach(function(key): void {
    setArg(key, defaults[key] === undefined ? false : defaults[key]);
  });

  let notFlags = [];

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
      const m = arg.match(/^--([^=]+)=([\s\S]*)$/);
      const key = m[1];
      let value = m[2];
      if (flags.bools[key]) {
        value = value !== "false";
      }
      setArg(key, value, arg);
    } else if (/^--no-.+/.test(arg)) {
      const key = arg.match(/^--no-(.+)/)[1];
      setArg(key, false, arg);
    } else if (/^--.+/.test(arg)) {
      const key = arg.match(/^--(.+)/)[1];
      const next = args[i + 1];
      if (
        next !== undefined &&
        !/^-/.test(next) &&
        !flags.bools[key] &&
        !flags.allBools &&
        (aliases[key] ? !aliasIsBoolean(key) : true)
      ) {
        setArg(key, next, arg);
        i++;
      } else if (/^(true|false)$/.test(next)) {
        setArg(key, next === "true", arg);
        i++;
      } else {
        setArg(key, flags.strings[key] ? "" : true, arg);
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
          setArg(letters[j], flags.strings[letters[j]] ? "" : true, arg);
        }
      }

      const key = arg.slice(-1)[0];
      if (!broken && key !== "-") {
        if (
          args[i + 1] &&
          !/^(-|--)[^-]/.test(args[i + 1]) &&
          !flags.bools[key] &&
          (aliases[key] ? !aliasIsBoolean(key) : true)
        ) {
          setArg(key, args[i + 1], arg);
          i++;
        } else if (args[i + 1] && /true|false/.test(args[i + 1])) {
          setArg(key, args[i + 1] === "true", arg);
          i++;
        } else {
          setArg(key, flags.strings[key] ? "" : true, arg);
        }
      }
    } else {
      if (!flags.unknownFn || flags.unknownFn(arg) !== false) {
        argv._.push(flags.strings["_"] || !isNumber(arg) ? arg : Number(arg));
      }
      if (options.stopEarly) {
        argv._.push.apply(argv._, args.slice(i + 1));
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
