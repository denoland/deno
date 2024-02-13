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

interface MediaTypeSpecificity extends Specificity {
  type: string;
  subtype: string;
  params: { [param: string]: string | undefined };
}

const simpleMediaTypeRegExp = /^\s*([^\s\/;]+)\/([^;\s]+)\s*(?:;(.*))?$/;

function quoteCount(str: string): number {
  let count = 0;
  let index = 0;

  while ((index = str.indexOf(`"`, index)) !== -1) {
    count++;
    index++;
  }

  return count;
}

function splitMediaTypes(accept: string): string[] {
  const accepts = accept.split(",");

  let j = 0;
  for (let i = 1; i < accepts.length; i++) {
    if (quoteCount(accepts[j]) % 2 === 0) {
      accepts[++j] = accepts[i];
    } else {
      accepts[j] += `,${accepts[i]}`;
    }
  }

  accepts.length = j + 1;

  return accepts;
}

function splitParameters(str: string): string[] {
  const parameters = str.split(";");

  let j = 0;
  for (let i = 1; i < parameters.length; i++) {
    if (quoteCount(parameters[j]) % 2 === 0) {
      parameters[++j] = parameters[i];
    } else {
      parameters[j] += `;${parameters[i]}`;
    }
  }

  parameters.length = j + 1;

  return parameters.map((p) => p.trim());
}

function splitKeyValuePair(str: string): [string, string | undefined] {
  const [key, value] = str.split("=");
  return [key.toLowerCase(), value];
}

function parseMediaType(
  str: string,
  i: number,
): MediaTypeSpecificity | undefined {
  const match = simpleMediaTypeRegExp.exec(str);

  if (!match) {
    return;
  }

  const params: { [param: string]: string | undefined } = Object.create(null);
  let q = 1;
  const [, type, subtype, parameters] = match;

  if (parameters) {
    const kvps = splitParameters(parameters).map(splitKeyValuePair);

    for (const [key, val] of kvps) {
      const value = val && val[0] === `"` && val[val.length - 1] === `"`
        ? val.slice(1, val.length - 1)
        : val;

      if (key === "q" && value) {
        q = parseFloat(value);
        break;
      }

      params[key] = value;
    }
  }

  return { type, subtype, params, q, i };
}

function parseAccept(accept: string): MediaTypeSpecificity[] {
  const accepts = splitMediaTypes(accept);

  const mediaTypes: MediaTypeSpecificity[] = [];
  for (let i = 0; i < accepts.length; i++) {
    const mediaType = parseMediaType(accepts[i].trim(), i);

    if (mediaType) {
      mediaTypes.push(mediaType);
    }
  }

  return mediaTypes;
}

function getFullType(spec: MediaTypeSpecificity) {
  return `${spec.type}/${spec.subtype}`;
}

function specify(
  type: string,
  spec: MediaTypeSpecificity,
  index: number,
): Specificity | undefined {
  const p = parseMediaType(type, index);

  if (!p) {
    return;
  }

  let s = 0;

  if (spec.type.toLowerCase() === p.type.toLowerCase()) {
    s |= 4;
  } else if (spec.type !== "*") {
    return;
  }

  if (spec.subtype.toLowerCase() === p.subtype.toLowerCase()) {
    s |= 2;
  } else if (spec.subtype !== "*") {
    return;
  }

  const keys = Object.keys(spec.params);
  if (keys.length) {
    if (
      keys.every((key) =>
        (spec.params[key] || "").toLowerCase() ===
          (p.params[key] || "").toLowerCase()
      )
    ) {
      s |= 1;
    } else {
      return;
    }
  }

  return {
    i: index,
    o: spec.o,
    q: spec.q,
    s,
  };
}

function getMediaTypePriority(
  type: string,
  accepted: MediaTypeSpecificity[],
  index: number,
) {
  let priority: Specificity = { o: -1, q: 0, s: 0, i: index };

  for (const accepts of accepted) {
    const spec = specify(type, accepts, index);

    if (
      spec &&
      ((priority.s || 0) - (spec.s || 0) ||
          (priority.q || 0) - (spec.q || 0) ||
          (priority.o || 0) - (spec.o || 0)) < 0
    ) {
      priority = spec;
    }
  }

  return priority;
}

export function preferredMediaTypes(
  accept?: string | null,
  provided?: string[],
): string[] {
  const accepts = parseAccept(accept === undefined ? "*/*" : accept || "");

  if (!provided) {
    return accepts
      .filter(isQuality)
      .sort(compareSpecs)
      .map(getFullType);
  }

  const priorities = provided.map((type, index) => {
    return getMediaTypePriority(type, accepts, index);
  });

  return priorities
    .filter(isQuality)
    .sort(compareSpecs)
    .map((priority) => provided[priorities.indexOf(priority)]);
}
