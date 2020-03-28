// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
export const LogLevel: Record<string, number> = {
  NOTSET: 0,
  DEBUG: 10,
  INFO: 20,
  WARNING: 30,
  ERROR: 40,
  CRITICAL: 50,
};

const byLevel = {
  [LogLevel.NOTSET]: "NOTSET",
  [LogLevel.DEBUG]: "DEBUG",
  [LogLevel.INFO]: "INFO",
  [LogLevel.WARNING]: "WARNING",
  [LogLevel.ERROR]: "ERROR",
  [LogLevel.CRITICAL]: "CRITICAL",
};

export function getLevelByName(name: string): number {
  return LogLevel[name];
}

export function getLevelName(level: number): string {
  return byLevel[level];
}
