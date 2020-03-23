// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

/* eslint-disable @typescript-eslint/no-explicit-any */
// TODO reenable this lint here

import { Queue, QueueImpl } from "./queue.ts";
import { isFiniteNonNegativeNumber } from "./shared-internals.ts";

export const queue_ = Symbol("queue_");
export const queueTotalSize_ = Symbol("queueTotalSize_");

export interface QueueElement<V> {
  value: V;
  size: number;
}

export interface QueueContainer<V> {
  [queue_]: Queue<QueueElement<V>>;
  [queueTotalSize_]: number;
}

export interface ByteQueueContainer {
  [queue_]: Queue<{
    buffer: ArrayBufferLike;
    byteOffset: number;
    byteLength: number;
  }>;
  [queueTotalSize_]: number;
}

export function dequeueValue<V>(container: QueueContainer<V>): V {
  // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
  // Assert: container.[[queue]] is not empty.
  const pair = container[queue_].shift()!;
  const newTotalSize = container[queueTotalSize_] - pair.size;
  container[queueTotalSize_] = Math.max(0, newTotalSize); // < 0 can occur due to rounding errors.
  return pair.value;
}

export function enqueueValueWithSize<V>(
  container: QueueContainer<V>,
  value: V,
  size: number
): void {
  // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
  if (!isFiniteNonNegativeNumber(size)) {
    throw new RangeError("Chunk size must be a non-negative, finite numbers");
  }
  container[queue_].push({ value, size });
  container[queueTotalSize_] += size;
}

export function peekQueueValue<V>(container: QueueContainer<V>): V {
  // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
  // Assert: container.[[queue]] is not empty.
  return container[queue_].front()!.value;
}

export function resetQueue<V>(
  container: ByteQueueContainer | QueueContainer<V>
): void {
  // Chrome (as of v67) has a steep performance cliff with large arrays
  // and shift(), around about 50k elements. While this is an unusual case
  // we use a simple wrapper around shift and push that is chunked to
  // avoid this pitfall.
  // @see: https://github.com/stardazed/sd-streams/issues/1
  container[queue_] = new QueueImpl<any>();

  // The code below can be used as a plain array implementation of the
  // Queue interface.
  // const q = [] as any;
  // q.front = function() { return this[0]; };
  // container[queue_] = q;

  container[queueTotalSize_] = 0;
}
