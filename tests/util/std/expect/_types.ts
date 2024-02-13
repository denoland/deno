// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

export interface MatcherContext {
  value: unknown;
  isNot: boolean;
  customMessage: string | undefined;
}

export type Matcher = (
  context: MatcherContext,
  ...args: any[]
) => MatchResult;

export type Matchers = {
  [key: string]: Matcher;
};
export type MatchResult = void | Promise<void> | boolean;
export type AnyConstructor = new (...args: any[]) => any;
