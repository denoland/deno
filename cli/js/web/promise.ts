// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export enum PromiseState {
  Pending,
  Fulfilled,
  Rejected,
}

export type PromiseDetails<T> = [PromiseState, T | undefined];
