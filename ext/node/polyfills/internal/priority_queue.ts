// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.

// The PriorityQueue is a basic implementation of a binary heap that accepts
// a custom sorting function via its constructor. This function is passed
// the two nodes to compare, similar to the native Array#sort. Crucially
// this enables priority queues that are based on a comparison of more than
// just a single criteria.

// deno-lint-ignore-file no-explicit-any
// deno-fmt-ignore-file
(function () {
  type Comparator<T> = (a: T, b: T) => number;
  type SetPosition<T> = (node: T, pos: number) => void;

  class PriorityQueue<T = any> {
    #compare: Comparator<T> = (a: any, b: any) => a - b;
    #heap: (T | undefined)[] = [undefined, undefined];
    #setPosition?: SetPosition<T>;
    #size = 0;

    constructor(comparator?: Comparator<T>, setPosition?: SetPosition<T>) {
      if (comparator !== undefined) {
        this.#compare = comparator;
      }
      if (setPosition !== undefined) {
        this.#setPosition = setPosition;
      }
    }

    insert(value: T): void {
      const heap = this.#heap;
      const pos = ++this.#size;
      heap[pos] = value;

      this.percolateUp(pos);
    }

    peek(): T | undefined {
      return this.#heap[1];
    }

    peekBottom(): T | undefined {
      return this.#heap[this.#size];
    }

    percolateDown(pos: number): void {
      const compare = this.#compare;
      const setPosition = this.#setPosition;
      const hasSetPosition = setPosition !== undefined;
      const heap = this.#heap;
      const size = this.#size;
      const hsize = size >> 1;
      const item = heap[pos] as T;

      while (pos <= hsize) {
        let child = pos << 1;
        const nextChild = child + 1;
        let childItem = heap[child] as T;

        if (nextChild <= size && compare(heap[nextChild] as T, childItem) < 0) {
          child = nextChild;
          childItem = heap[nextChild] as T;
        }

        if (compare(item, childItem) <= 0) break;

        if (hasSetPosition) {
          setPosition(childItem, pos);
        }

        heap[pos] = childItem;
        pos = child;
      }

      heap[pos] = item;
      if (hasSetPosition) {
        setPosition(item, pos);
      }
    }

    percolateUp(pos: number): void {
      const heap = this.#heap;
      const compare = this.#compare;
      const setPosition = this.#setPosition;
      const hasSetPosition = setPosition !== undefined;
      const item = heap[pos] as T;

      while (pos > 1) {
        const parent = pos >> 1;
        const parentItem = heap[parent] as T;
        if (compare(parentItem, item) <= 0) {
          break;
        }
        heap[pos] = parentItem;
        if (hasSetPosition) {
          setPosition(parentItem, pos);
        }
        pos = parent;
      }

      heap[pos] = item;
      if (hasSetPosition) {
        setPosition(item, pos);
      }
    }

    removeAt(pos: number): void {
      const heap = this.#heap;
      let size = this.#size;
      heap[pos] = heap[size];
      heap[size] = undefined;
      size = --this.#size;

      if (size > 0 && pos <= size) {
        if (pos > 1 && this.#compare(heap[pos >> 1] as T, heap[pos] as T) > 0) {
          this.percolateUp(pos);
        } else {
          this.percolateDown(pos);
        }
      }
    }

    shift(): T | undefined {
      const heap = this.#heap;
      const value = heap[1];
      if (value === undefined) {
        return undefined;
      }

      this.removeAt(1);

      return value;
    }
  }

  return {
    PriorityQueue,
    default: PriorityQueue,
  };
})()
