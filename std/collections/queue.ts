// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { Deferred, deferred } from "../async/deferred.ts";

class QueueItem<T> {
  val: T;
  next: QueueItem<T> | undefined;

  constructor(val: T) {
    this.val = val;
    this.next = undefined;
  }
}

export class Queue<T> {
  #size = 0;
  #head: QueueItem<T> | undefined = undefined;
  #tail: QueueItem<T> | undefined = undefined;
  #closed = false;
  #dataIsAvailable: Deferred<void> | undefined = undefined;

  /** Add value to the back of the queue.  Returns the value passed in. */
  add(val: T): T {
    if (this.#closed) {
      throw new Error("Queue is closed.");
    }

    const node = new QueueItem(val);
    if (this.#size === 0) {
      this.#head = node;
      this.#tail = node;
    } else {
      this.#tail!.next = node;
      this.#tail = node;
    }
    this.#size++;

    this.#dataIsAvailable?.resolve();
    return val;
  }

  /** Remove and return the value at the front of the queue. */
  remove(): T | undefined {
    if (this.#size === 0) {
      return undefined;
    }

    this.#size--;
    const ret: QueueItem<T> = this.#head!;
    this.#head = this.#head!.next;

    if (this.#size === 0) {
      this.#tail = undefined;
    }

    return ret.val;
  }

  /** Return the number of items in the queue */
  size(): number {
    return this.#size;
  }

  /** Returns true if the queue is empty */
  isEmpty(): boolean {
    return this.#size === 0;
  }

  /** Peek at the value at the front of the queue without removing it */
  peek(): T | undefined {
    return this.#head?.val;
  }

  /** Resets the queue to an empty initial state */
  reset(): void {
    this.#head = undefined;
    this.#tail = undefined;
    this.#size = 0;
    this.#closed = false;
    this.#dataIsAvailable = undefined;
  }

  /** Close this queue.  No further data can be accepted. */
  close(): void {
    this.#closed = true;
  }

  /** Iterate over the queue, removing (and returning) each item in the queue. */
  drain(): IterableIterator<T> {
    return {
      next: (): IteratorResult<T> => {
        if (this.size() === 0) {
          return { value: undefined, done: true };
        }

        return { value: this.remove()!, done: false };
      },

      [Symbol.iterator](): IterableIterator<T> {
        return this;
      },
    };
  }

  /** Iterate over the queue, removing (and returning) each item in the queue.
   * The iterator does not complete when the queue is empty, but remains active
   * (but paused) until new data arrives in the queue at which point it will
   * resume draining again.  This is repeated indefinitely until the queue is
   * closed.
   */
  drainAndWait(): AsyncIterableIterator<T> {
    const setupIterator = (): void => {
      if (this.#dataIsAvailable === undefined) {
        this.#dataIsAvailable = deferred();
      }
    };

    return {
      next: async (): Promise<IteratorResult<T>> => {
        if (this.size() === 0) {
          if (this.#closed) {
            return { value: undefined, done: true };
          } else {
            await this.#dataIsAvailable;
            this.#dataIsAvailable = deferred();
          }
        }
        const finished = this.size() === 0 && this.#closed;
        return { value: this.remove()!, done: finished };
      },

      [Symbol.asyncIterator](): AsyncIterableIterator<T> {
        setupIterator();
        return this;
      },
    };
  }
}
