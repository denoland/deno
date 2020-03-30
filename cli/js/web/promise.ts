export enum PromiseState {
  Pending = 0,
  Fulfilled = 1,
  Rejected = 2,
}

export type PromiseDetails<T> = [PromiseState, T | undefined];
