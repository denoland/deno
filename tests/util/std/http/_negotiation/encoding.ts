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

interface EncodingSpecificity extends Specificity {
  encoding?: string;
}

const simpleEncodingRegExp = /^\s*([^\s;]+)\s*(?:;(.*))?$/;

function parseEncoding(
  str: string,
  i: number,
): EncodingSpecificity | undefined {
  const match = simpleEncodingRegExp.exec(str);
  if (!match) {
    return undefined;
  }

  const encoding = match[1];
  let q = 1;
  if (match[2]) {
    const params = match[2].split(";");
    for (const param of params) {
      const p = param.trim().split("=");
      if (p[0] === "q") {
        q = parseFloat(p[1]);
        break;
      }
    }
  }

  return { encoding, q, i };
}

function specify(
  encoding: string,
  spec: EncodingSpecificity,
  i = -1,
): Specificity | undefined {
  if (!spec.encoding) {
    return;
  }
  let s = 0;
  if (spec.encoding.toLocaleLowerCase() === encoding.toLocaleLowerCase()) {
    s = 1;
  } else if (spec.encoding !== "*") {
    return;
  }

  return {
    i,
    o: spec.i,
    q: spec.q,
    s,
  };
}

function parseAcceptEncoding(accept: string): EncodingSpecificity[] {
  const accepts = accept.split(",");
  const parsedAccepts: EncodingSpecificity[] = [];
  let hasIdentity = false;
  let minQuality = 1;

  for (let i = 0; i < accepts.length; i++) {
    const encoding = parseEncoding(accepts[i].trim(), i);

    if (encoding) {
      parsedAccepts.push(encoding);
      hasIdentity = hasIdentity || !!specify("identity", encoding);
      minQuality = Math.min(minQuality, encoding.q || 1);
    }
  }

  if (!hasIdentity) {
    parsedAccepts.push({
      encoding: "identity",
      q: minQuality,
      i: accepts.length - 1,
    });
  }

  return parsedAccepts;
}

function getEncodingPriority(
  encoding: string,
  accepted: Specificity[],
  index: number,
): Specificity {
  let priority: Specificity = { o: -1, q: 0, s: 0, i: 0 };

  for (const s of accepted) {
    const spec = specify(encoding, s, index);

    if (
      spec &&
      (priority.s! - spec.s! || priority.q - spec.q ||
          priority.o! - spec.o!) <
        0
    ) {
      priority = spec;
    }
  }

  return priority;
}

/** Given an `Accept-Encoding` string, parse out the encoding returning a
 * negotiated encoding based on the `provided` encodings otherwise just a
 * prioritized array of encodings. */
export function preferredEncodings(
  accept: string,
  provided?: string[],
): string[] {
  const accepts = parseAcceptEncoding(accept);

  if (!provided) {
    return accepts
      .filter(isQuality)
      .sort(compareSpecs)
      .map((spec) => spec.encoding!);
  }

  const priorities = provided.map((type, index) =>
    getEncodingPriority(type, accepts, index)
  );

  return priorities
    .filter(isQuality)
    .sort(compareSpecs)
    .map((priority) => provided[priorities.indexOf(priority)]);
}
