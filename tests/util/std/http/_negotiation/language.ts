// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/*!
 * Adapted directly from negotiator at https://github.com/jshttp/negotiator/
 * which is licensed as follows:
 *
 * (The MIT License)
 *
 * Copyright (c) 2012-2014 Federico Romero
 * Copyright (c) 2012-2014 Isaac Z. Schlueter
 * Copyright (c) 2014-2015 Douglas Christopher Wilson
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files (the
 * 'Software'), to deal in the Software without restriction, including
 * without limitation the rights to use, copy, modify, merge, publish,
 * distribute, sublicense, and/or sell copies of the Software, and to
 * permit persons to whom the Software is furnished to do so, subject to
 * the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
 * IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
 * TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
 * SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */

import { compareSpecs, isQuality, Specificity } from "./common.ts";

interface LanguageSpecificity extends Specificity {
  prefix: string;
  suffix?: string;
  full: string;
}

const SIMPLE_LANGUAGE_REGEXP = /^\s*([^\s\-;]+)(?:-([^\s;]+))?\s*(?:;(.*))?$/;

function parseLanguage(
  str: string,
  i: number,
): LanguageSpecificity | undefined {
  const match = SIMPLE_LANGUAGE_REGEXP.exec(str);
  if (!match) {
    return undefined;
  }

  const [, prefix, suffix] = match;
  const full = suffix ? `${prefix}-${suffix}` : prefix;

  let q = 1;
  if (match[3]) {
    const params = match[3].split(";");
    for (const param of params) {
      const [key, value] = param.trim().split("=");
      if (key === "q") {
        q = parseFloat(value);
        break;
      }
    }
  }

  return { prefix, suffix, full, q, i };
}

function parseAcceptLanguage(accept: string): LanguageSpecificity[] {
  const accepts = accept.split(",");
  const result: LanguageSpecificity[] = [];

  for (let i = 0; i < accepts.length; i++) {
    const language = parseLanguage(accepts[i].trim(), i);
    if (language) {
      result.push(language);
    }
  }
  return result;
}

function specify(
  language: string,
  spec: LanguageSpecificity,
  i: number,
): Specificity | undefined {
  const p = parseLanguage(language, i);
  if (!p) {
    return undefined;
  }
  let s = 0;
  if (spec.full.toLowerCase() === p.full.toLowerCase()) {
    s |= 4;
  } else if (spec.prefix.toLowerCase() === p.prefix.toLowerCase()) {
    s |= 2;
  } else if (spec.full.toLowerCase() === p.prefix.toLowerCase()) {
    s |= 1;
  } else if (spec.full !== "*") {
    return;
  }

  return { i, o: spec.i, q: spec.q, s };
}

function getLanguagePriority(
  language: string,
  accepted: LanguageSpecificity[],
  index: number,
): Specificity {
  let priority: Specificity = { i: -1, o: -1, q: 0, s: 0 };
  for (const accepts of accepted) {
    const spec = specify(language, accepts, index);
    if (
      spec &&
      ((priority.s ?? 0) - (spec.s ?? 0) || priority.q - spec.q ||
          (priority.o ?? 0) - (spec.o ?? 0)) < 0
    ) {
      priority = spec;
    }
  }
  return priority;
}

export function preferredLanguages(
  accept = "*",
  provided?: string[],
): string[] {
  const accepts = parseAcceptLanguage(accept);

  if (!provided) {
    return accepts
      .filter(isQuality)
      .sort(compareSpecs)
      .map((spec) => spec.full);
  }

  const priorities = provided
    .map((type, index) => getLanguagePriority(type, accepts, index));

  return priorities
    .filter(isQuality)
    .sort(compareSpecs)
    .map((priority) => provided[priorities.indexOf(priority)]);
}
