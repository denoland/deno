// Copyright 2018-2025 the Deno authors. MIT license.
const {
  op_async_barrier_create,
  op_async_barrier_await,
  op_async_yield,
  op_async_spin_on_state,
  op_stats_capture,
  op_stats_diff,
  op_stats_dump,
  op_stats_delete,
  op_async_never_resolves,
  op_async_fake,
  op_async_promise_id,
} = Deno
  .core
  .ops;

export function barrierCreate(name: string, count: number) {
  op_async_barrier_create(name, count);
}

export function barrierAwait(name: string) {
  return op_async_barrier_await(name);
}

export async function asyncYield() {
  await op_async_yield();
}

// This function never returns.
export async function asyncSpin() {
  await op_async_spin_on_state();
}

export function asyncNeverResolves() {
  const prom = op_async_never_resolves();
  Deno.core.refOpPromise(prom);
  return prom;
}

let nextStats = 0;

export function fakeAsync() {
  return op_async_fake();
}

export function asyncPromiseId(): number {
  return op_async_promise_id();
}

export class Stats {
  constructor(public name: string) {
    op_stats_capture(this.name);
  }

  dump(): StatsCollection {
    return new StatsCollection(op_stats_dump(this.name).active);
  }

  [Symbol.dispose]() {
    op_stats_delete(this.name);
  }
}

export class StatsDiff {
  #appeared;
  #disappeared;

  // deno-lint-ignore no-explicit-any
  constructor(private diff: any) {
    this.#appeared = new StatsCollection(this.diff.appeared);
    this.#disappeared = new StatsCollection(this.diff.disappeared);
  }

  get empty(): boolean {
    return this.#appeared.empty && this.#disappeared.empty;
  }

  get appeared(): StatsCollection {
    return this.#appeared;
  }

  get disappeared(): StatsCollection {
    return this.#disappeared;
  }
}

export enum LeakType {
  AsyncOp = "AsyncOp",
  Resource = "Resource",
  Timer = "Timer",
  Interval = "Interval",
}

// This contains an array of serialized RuntimeActivity structs.
export class StatsCollection {
  // deno-lint-ignore no-explicit-any
  constructor(private data: any[]) {
    console.log(data);
  }

  count(...types: LeakType[]): number {
    let count = 0;
    for (const item of this.data) {
      for (const type of types) {
        if (type in item) {
          count++;
        }
      }
    }
    return count;
  }

  countWithTraces(...types: LeakType[]): number {
    let count = 0;
    for (const item of this.data) {
      for (const type of types) {
        if (type in item) {
          // Make sure it's a non-empty stack trace
          if (
            typeof item[type][1] === "string" &&
            item[type][1].trim().length > 0
          ) {
            count++;
          }
        }
      }
    }
    return count;
  }

  get rawData() {
    return this.data;
  }

  get empty(): boolean {
    return this.data.length == 0;
  }
}

export class StatsFactory {
  static capture(): Stats {
    return new Stats(`stats-${nextStats++}`);
  }

  static diff(before: Stats, after: Stats): StatsDiff {
    return new StatsDiff(op_stats_diff(before.name, after.name));
  }
}
