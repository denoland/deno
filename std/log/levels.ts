// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
/** Get log level numeric values through enum constants
 */
export enum LogLevels {
  NOTSET = 0,
  DEBUG = 10,
  INFO = 20,
  WARNING = 30,
  ERROR = 40,
  CRITICAL = 50,
}

/** Permitted log level names */
export const LogLevelNames = Object.keys(LogLevels).filter((key) =>
  isNaN(Number(key))
);

/** Union of valid log level strings */
export type LevelName = keyof typeof LogLevels;

const byLevel: Record<string, LevelName> = {
  [String(LogLevels.NOTSET)]: "NOTSET",
  [String(LogLevels.DEBUG)]: "DEBUG",
  [String(LogLevels.INFO)]: "INFO",
  [String(LogLevels.WARNING)]: "WARNING",
  [String(LogLevels.ERROR)]: "ERROR",
  [String(LogLevels.CRITICAL)]: "CRITICAL",
};

/** Returns the numeric log level associated with the passed,
 * stringy log level name.
 */
export function getLevelByName(name: LevelName): number {
  switch (name) {
    case "NOTSET":
      return LogLevels.NOTSET;
    case "DEBUG":
      return LogLevels.DEBUG;
    case "INFO":
      return LogLevels.INFO;
    case "WARNING":
      return LogLevels.WARNING;
    case "ERROR":
      return LogLevels.ERROR;
    case "CRITICAL":
      return LogLevels.CRITICAL;
    default:
      throw new Error(`no log level found for "${name}"`);
  }
}

/** Returns the stringy log level name provided the numeric log level */
export function getLevelName(level: number): LevelName {
  const levelName = byLevel[level];
  if (levelName) {
    return levelName;
  }
  throw new Error(`no level name found for level: ${level}`);
}
