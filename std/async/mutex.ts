// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { deferred } from "./deferred.ts";

export interface Locker {
  lock: () => Promise<void>;
  unlock: () => void;
}

export class Mutex implements Locker {
  private queue: Array<() => void> = []; //currently waiting Locks
  private locked = false;

  /** Request a lock.  It is the callers responsibility to await this. */
  lock(): Promise<void> {
    if (this.locked) {
      const retProm = deferred<void>();
      this.queue.push(function (): void {
        retProm.resolve();
      });
      return retProm;
    } else {
      this.locked = true;
      return Promise.resolve();
    }
  }

  /** Release the lock. */
  unlock(): void {
    if (this.locked) {
      if (this.queue.length > 0) {
        this.queue[0]();
        this.queue.splice(0, 1);
      } else {
        this.locked = false;
      }
    } else {
      throw new Error("not locked");
    }
  }

  private static mus: {
    [key: string]: Mutex;
  } = {};

  /**
   *  Execute an async function that cannot be interrupted by 
   *  functions requesting a lock of the same name.  
   *
   *  Example usage: 
   *
   *      await doAtomic("name_of_lock", async function(){
   *        const dat = await read_from_shared_resource();
   *        await write_to_shared_resource(dat + 1);
   *      )};
   */
  static async doAtomic(
    name: string,
    cb: () => Promise<void>,
  ): Promise<void> {
    if (!Mutex.mus[name]) {
      Mutex.mus[name] = new Mutex();
    }
    await Mutex.mus[name].lock();
    try {
      await cb();
    } finally {
      Mutex.mus[name].unlock();
      if (!Mutex.mus[name].locked) { //nobody waiting?
        delete Mutex.mus[name];
      }
    }
  }
}
