// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
export const LogLevel = {
  NOTSET: 0,
  DEBUG: 10,
  INFO: 20,
  WARNING: 30,
  ERROR: 40,
  CRITICAL: 50
};

const byName = {
  NOTSET: LogLevel.NOTSET,
  DEBUG: LogLevel.DEBUG,
  INFO: LogLevel.INFO,
  WARNING: LogLevel.WARNING,
  ERROR: LogLevel.ERROR,
  CRITICAL: LogLevel.CRITICAL
};

const byLevel = {
  [LogLevel.NOTSET]: "NOTSET",
  [LogLevel.DEBUG]: "DEBUG",
  [LogLevel.INFO]: "INFO",
  [LogLevel.WARNING]: "WARNING",
  [LogLevel.ERROR]: "ERROR",
  [LogLevel.CRITICAL]: "CRITICAL"
};

export function getLevelByName(name: string): number {
  return byName[name];
}

export function getLevelName(level: number): string {
  return byLevel[level];
}
