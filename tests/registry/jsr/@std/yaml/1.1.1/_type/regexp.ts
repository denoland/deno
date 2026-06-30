// Ported and adapted from js-yaml-js-types v1.0.0:
// https://github.com/nodeca/js-yaml-js-types/tree/ac537e7bbdd3c2cbbd9882ca3919c520c2dc022b
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

import type { Type } from "../_type.ts";

const REGEXP = /^\/(?<regexp>[\s\S]+)\/(?<modifiers>[gismuy]*)$/;

export const regexp: Type<"scalar", RegExp> = {
  tag: "tag:yaml.org,2002:js/regexp",
  kind: "scalar",
  resolve(data: string | null): boolean {
    if (data === null || !data.length) return false;

    if (data.charAt(0) === "/") {
      // Ensure regex is properly terminated
      const groups = data.match(REGEXP)?.groups;
      if (!groups) return false;
      // Check no duplicate modifiers
      const modifiers = groups.modifiers ?? "";
      if (new Set(modifiers).size < modifiers.length) return false;
    }

    return true;
  },
  construct(data: string): RegExp {
    const { regexp = data, modifiers = "" } = data.match(REGEXP)?.groups ?? {};
    return new RegExp(regexp, modifiers);
  },
  predicate: (object: unknown): object is RegExp => object instanceof RegExp,
  represent: (object: RegExp): string => object.toString(),
};
