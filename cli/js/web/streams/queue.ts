// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

const CHUNK_SIZE = 16384;

export interface Queue<T> {
  push(t: T): void;
  shift(): T | undefined;
  front(): T | undefined;
  readonly length: number;
}

export class QueueImpl<T> implements Queue<T> {
  private readonly chunks_: T[][];
  private readChunk_: T[];
  private writeChunk_: T[];
  private length_: number;

  constructor() {
    this.chunks_ = [[]];
    this.readChunk_ = this.writeChunk_ = this.chunks_[0];
    this.length_ = 0;
  }

  push(t: T): void {
    this.writeChunk_.push(t);
    this.length_ += 1;
    if (this.writeChunk_.length === CHUNK_SIZE) {
      this.writeChunk_ = [];
      this.chunks_.push(this.writeChunk_);
    }
  }

  front(): T | undefined {
    if (this.length_ === 0) {
      return undefined;
    }
    return this.readChunk_[0];
  }

  shift(): T | undefined {
    if (this.length_ === 0) {
      return undefined;
    }
    const t = this.readChunk_.shift();

    this.length_ -= 1;
    if (this.readChunk_.length === 0 && this.readChunk_ !== this.writeChunk_) {
      this.chunks_.shift();
      this.readChunk_ = this.chunks_[0];
    }
    return t;
  }

  get length(): number {
    return this.length_;
  }
}
