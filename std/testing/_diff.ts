// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

interface FarthestPoint {
  y: number;
  id: number;
}

export enum DiffType {
  removed = "removed",
  common = "common",
  added = "added",
}

export interface DiffResult<T> {
  type: DiffType;
  value: T;
}

const REMOVED = 1;
const COMMON = 2;
const ADDED = 3;

function createCommon<T>(A: T[], B: T[], reverse?: boolean): T[] {
  const common = [];
  if (A.length === 0 || B.length === 0) return [];
  for (let i = 0; i < Math.min(A.length, B.length); i += 1) {
    if (
      A[reverse ? A.length - i - 1 : i] === B[reverse ? B.length - i - 1 : i]
    ) {
      common.push(A[reverse ? A.length - i - 1 : i]);
    } else {
      return common;
    }
  }
  return common;
}

/**
 * Renders the differences between the actual and expected values
 * @param A Actual value
 * @param B Expected value
 */
export function diff<T>(A: T[], B: T[]): Array<DiffResult<T>> {
  const prefixCommon = createCommon(A, B);
  const suffixCommon = createCommon(
    A.slice(prefixCommon.length),
    B.slice(prefixCommon.length),
    true,
  ).reverse();
  A = suffixCommon.length
    ? A.slice(prefixCommon.length, -suffixCommon.length)
    : A.slice(prefixCommon.length);
  B = suffixCommon.length
    ? B.slice(prefixCommon.length, -suffixCommon.length)
    : B.slice(prefixCommon.length);
  const swapped = B.length > A.length;
  [A, B] = swapped ? [B, A] : [A, B];
  const M = A.length;
  const N = B.length;
  if (!M && !N && !suffixCommon.length && !prefixCommon.length) return [];
  if (!N) {
    return [
      ...prefixCommon.map(
        (c): DiffResult<typeof c> => ({ type: DiffType.common, value: c }),
      ),
      ...A.map(
        (a): DiffResult<typeof a> => ({
          type: swapped ? DiffType.added : DiffType.removed,
          value: a,
        }),
      ),
      ...suffixCommon.map(
        (c): DiffResult<typeof c> => ({ type: DiffType.common, value: c }),
      ),
    ];
  }
  const offset = N;
  const delta = M - N;
  const size = M + N + 1;
  const fp = new Array(size).fill({ y: -1 });
  /**
   * INFO:
   * This buffer is used to save memory and improve performance.
   * The first half is used to save route and last half is used to save diff
   * type.
   * This is because, when I kept new uint8array area to save type,performance
   * worsened.
   */
  const routes = new Uint32Array((M * N + size + 1) * 2);
  const diffTypesPtrOffset = routes.length / 2;
  let ptr = 0;
  let p = -1;

  function backTrace<T>(
    A: T[],
    B: T[],
    current: FarthestPoint,
    swapped: boolean,
  ): Array<{
    type: DiffType;
    value: T;
  }> {
    const M = A.length;
    const N = B.length;
    const result = [];
    let a = M - 1;
    let b = N - 1;
    let j = routes[current.id];
    let type = routes[current.id + diffTypesPtrOffset];
    while (true) {
      if (!j && !type) break;
      const prev = j;
      if (type === REMOVED) {
        result.unshift({
          type: swapped ? DiffType.removed : DiffType.added,
          value: B[b],
        });
        b -= 1;
      } else if (type === ADDED) {
        result.unshift({
          type: swapped ? DiffType.added : DiffType.removed,
          value: A[a],
        });
        a -= 1;
      } else {
        result.unshift({ type: DiffType.common, value: A[a] });
        a -= 1;
        b -= 1;
      }
      j = routes[prev];
      type = routes[prev + diffTypesPtrOffset];
    }
    return result;
  }

  function createFP(
    slide: FarthestPoint,
    down: FarthestPoint,
    k: number,
    M: number,
  ): FarthestPoint {
    if (slide && slide.y === -1 && down && down.y === -1) {
      return { y: 0, id: 0 };
    }
    if (
      (down && down.y === -1) ||
      k === M ||
      (slide && slide.y) > (down && down.y) + 1
    ) {
      const prev = slide.id;
      ptr++;
      routes[ptr] = prev;
      routes[ptr + diffTypesPtrOffset] = ADDED;
      return { y: slide.y, id: ptr };
    } else {
      const prev = down.id;
      ptr++;
      routes[ptr] = prev;
      routes[ptr + diffTypesPtrOffset] = REMOVED;
      return { y: down.y + 1, id: ptr };
    }
  }

  function snake<T>(
    k: number,
    slide: FarthestPoint,
    down: FarthestPoint,
    _offset: number,
    A: T[],
    B: T[],
  ): FarthestPoint {
    const M = A.length;
    const N = B.length;
    if (k < -N || M < k) return { y: -1, id: -1 };
    const fp = createFP(slide, down, k, M);
    while (fp.y + k < M && fp.y < N && A[fp.y + k] === B[fp.y]) {
      const prev = fp.id;
      ptr++;
      fp.id = ptr;
      fp.y += 1;
      routes[ptr] = prev;
      routes[ptr + diffTypesPtrOffset] = COMMON;
    }
    return fp;
  }

  while (fp[delta + offset].y < N) {
    p = p + 1;
    for (let k = -p; k < delta; ++k) {
      fp[k + offset] = snake(
        k,
        fp[k - 1 + offset],
        fp[k + 1 + offset],
        offset,
        A,
        B,
      );
    }
    for (let k = delta + p; k > delta; --k) {
      fp[k + offset] = snake(
        k,
        fp[k - 1 + offset],
        fp[k + 1 + offset],
        offset,
        A,
        B,
      );
    }
    fp[delta + offset] = snake(
      delta,
      fp[delta - 1 + offset],
      fp[delta + 1 + offset],
      offset,
      A,
      B,
    );
  }
  return [
    ...prefixCommon.map(
      (c): DiffResult<typeof c> => ({ type: DiffType.common, value: c }),
    ),
    ...backTrace(A, B, fp[delta + offset], swapped),
    ...suffixCommon.map(
      (c): DiffResult<typeof c> => ({ type: DiffType.common, value: c }),
    ),
  ];
}
