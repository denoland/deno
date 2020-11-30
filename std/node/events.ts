// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2019 Denolibs authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import { assert } from "../_util/assert.ts";
import { validateIntegerRange } from "./_utils.ts";

// deno-lint-ignore no-explicit-any
export type GenericFunction = (...args: any[]) => any;

export interface WrappedFunction extends Function {
  listener: GenericFunction;
}

// deno-lint-ignore no-explicit-any
function createIterResult(value: any, done: boolean): IteratorResult<any> {
  return { value, done };
}

interface AsyncIterable {
  // deno-lint-ignore no-explicit-any
  next(): Promise<IteratorResult<any, any>>;
  // deno-lint-ignore no-explicit-any
  return(): Promise<IteratorResult<any, any>>;
  throw(err: Error): void;
  // deno-lint-ignore no-explicit-any
  [Symbol.asyncIterator](): any;
}

export let defaultMaxListeners = 10;

/**
 * See also https://nodejs.org/api/events.html
 */
export default class EventEmitter {
  public static captureRejectionSymbol = Symbol.for("nodejs.rejection");
  public static errorMonitor = Symbol("events.errorMonitor");
  public static get defaultMaxListeners() {
    return defaultMaxListeners;
  }
  public static set defaultMaxListeners(value: number) {
    defaultMaxListeners = value;
  }

  private maxListeners: number | undefined;
  private _events: Map<
    string | symbol,
    Array<GenericFunction | WrappedFunction>
  >;

  public constructor() {
    this._events = new Map();
  }

  private _addListener(
    eventName: string | symbol,
    listener: GenericFunction | WrappedFunction,
    prepend: boolean,
  ): this {
    this.emit("newListener", eventName, listener);
    if (this._events.has(eventName)) {
      const listeners = this._events.get(eventName) as Array<
        GenericFunction | WrappedFunction
      >;
      if (prepend) {
        listeners.unshift(listener);
      } else {
        listeners.push(listener);
      }
    } else {
      this._events.set(eventName, [listener]);
    }
    const max = this.getMaxListeners();
    if (max > 0 && this.listenerCount(eventName) > max) {
      const warning = new Error(
        `Possible EventEmitter memory leak detected.
         ${this.listenerCount(eventName)} ${eventName.toString()} listeners.
         Use emitter.setMaxListeners() to increase limit`,
      );
      warning.name = "MaxListenersExceededWarning";
      console.warn(warning);
    }

    return this;
  }

  /** Alias for emitter.on(eventName, listener). */
  public addListener(
    eventName: string | symbol,
    listener: GenericFunction | WrappedFunction,
  ): this {
    return this._addListener(eventName, listener, false);
  }

  /**
   * Synchronously calls each of the listeners registered for the event named
   * eventName, in the order they were registered, passing the supplied
   * arguments to each.
   * @return true if the event had listeners, false otherwise
   */
  // deno-lint-ignore no-explicit-any
  public emit(eventName: string | symbol, ...args: any[]): boolean {
    if (this._events.has(eventName)) {
      if (
        eventName === "error" &&
        this._events.get(EventEmitter.errorMonitor)
      ) {
        this.emit(EventEmitter.errorMonitor, ...args);
      }
      const listeners = (this._events.get(
        eventName,
      ) as GenericFunction[]).slice(); // We copy with slice() so array is not mutated during emit
      for (const listener of listeners) {
        try {
          listener.apply(this, args);
        } catch (err) {
          this.emit("error", err);
        }
      }
      return true;
    } else if (eventName === "error") {
      if (this._events.get(EventEmitter.errorMonitor)) {
        this.emit(EventEmitter.errorMonitor, ...args);
      }
      const errMsg = args.length > 0 ? args[0] : Error("Unhandled error.");
      throw errMsg;
    }
    return false;
  }

  /**
   * Returns an array listing the events for which the emitter has
   * registered listeners.
   */
  public eventNames(): [string | symbol] {
    return Array.from(this._events.keys()) as [string | symbol];
  }

  /**
   * Returns the current max listener value for the EventEmitter which is
   * either set by emitter.setMaxListeners(n) or defaults to
   * EventEmitter.defaultMaxListeners.
   */
  public getMaxListeners(): number {
    return this.maxListeners || EventEmitter.defaultMaxListeners;
  }

  /**
   * Returns the number of listeners listening to the event named
   * eventName.
   */
  public listenerCount(eventName: string | symbol): number {
    if (this._events.has(eventName)) {
      return (this._events.get(eventName) as GenericFunction[]).length;
    } else {
      return 0;
    }
  }

  private _listeners(
    target: EventEmitter,
    eventName: string | symbol,
    unwrap: boolean,
  ): GenericFunction[] {
    if (!target._events.has(eventName)) {
      return [];
    }
    const eventListeners = target._events.get(eventName) as GenericFunction[];

    return unwrap
      ? this.unwrapListeners(eventListeners)
      : eventListeners.slice(0);
  }

  private unwrapListeners(arr: GenericFunction[]): GenericFunction[] {
    const unwrappedListeners = new Array(arr.length) as GenericFunction[];
    for (let i = 0; i < arr.length; i++) {
      // deno-lint-ignore no-explicit-any
      unwrappedListeners[i] = (arr[i] as any)["listener"] || arr[i];
    }
    return unwrappedListeners;
  }

  /** Returns a copy of the array of listeners for the event named eventName.*/
  public listeners(eventName: string | symbol): GenericFunction[] {
    return this._listeners(this, eventName, true);
  }

  /**
   * Returns a copy of the array of listeners for the event named eventName,
   * including any wrappers (such as those created by .once()).
   */
  public rawListeners(
    eventName: string | symbol,
  ): Array<GenericFunction | WrappedFunction> {
    return this._listeners(this, eventName, false);
  }

  /** Alias for emitter.removeListener(). */
  public off(eventName: string | symbol, listener: GenericFunction): this {
    return this.removeListener(eventName, listener);
  }

  /**
   * Adds the listener function to the end of the listeners array for the event
   *  named eventName. No checks are made to see if the listener has already
   * been added. Multiple calls passing the same combination of eventName and
   * listener will result in the listener being added, and called, multiple
   * times.
   */
  public on(
    eventName: string | symbol,
    listener: GenericFunction | WrappedFunction,
  ): this {
    return this._addListener(eventName, listener, false);
  }

  /**
   * Adds a one-time listener function for the event named eventName. The next
   * time eventName is triggered, this listener is removed and then invoked.
   */
  public once(eventName: string | symbol, listener: GenericFunction): this {
    const wrapped: WrappedFunction = this.onceWrap(eventName, listener);
    this.on(eventName, wrapped);
    return this;
  }

  // Wrapped function that calls EventEmitter.removeListener(eventName, self) on execution.
  private onceWrap(
    eventName: string | symbol,
    listener: GenericFunction,
  ): WrappedFunction {
    const wrapper = function (
      this: {
        eventName: string | symbol;
        listener: GenericFunction;
        rawListener: GenericFunction | WrappedFunction;
        context: EventEmitter;
      },
      // deno-lint-ignore no-explicit-any
      ...args: any[]
    ): void {
      this.context.removeListener(
        this.eventName,
        this.rawListener as GenericFunction,
      );
      this.listener.apply(this.context, args);
    };
    const wrapperContext = {
      eventName: eventName,
      listener: listener,
      rawListener: (wrapper as unknown) as WrappedFunction,
      context: this,
    };
    const wrapped = (wrapper.bind(
      wrapperContext,
    ) as unknown) as WrappedFunction;
    wrapperContext.rawListener = wrapped;
    wrapped.listener = listener;
    return wrapped as WrappedFunction;
  }

  /**
   * Adds the listener function to the beginning of the listeners array for the
   *  event named eventName. No checks are made to see if the listener has
   * already been added. Multiple calls passing the same combination of
   * eventName and listener will result in the listener being added, and
   * called, multiple times.
   */
  public prependListener(
    eventName: string | symbol,
    listener: GenericFunction | WrappedFunction,
  ): this {
    return this._addListener(eventName, listener, true);
  }

  /**
   * Adds a one-time listener function for the event named eventName to the
   * beginning of the listeners array. The next time eventName is triggered,
   * this listener is removed, and then invoked.
   */
  public prependOnceListener(
    eventName: string | symbol,
    listener: GenericFunction,
  ): this {
    const wrapped: WrappedFunction = this.onceWrap(eventName, listener);
    this.prependListener(eventName, wrapped);
    return this;
  }

  /** Removes all listeners, or those of the specified eventName. */
  public removeAllListeners(eventName?: string | symbol): this {
    if (this._events === undefined) {
      return this;
    }

    if (eventName) {
      if (this._events.has(eventName)) {
        const listeners = (this._events.get(eventName) as Array<
          GenericFunction | WrappedFunction
        >).slice(); // Create a copy; We use it AFTER it's deleted.
        this._events.delete(eventName);
        for (const listener of listeners) {
          this.emit("removeListener", eventName, listener);
        }
      }
    } else {
      const eventList: [string | symbol] = this.eventNames();
      eventList.map((value: string | symbol) => {
        this.removeAllListeners(value);
      });
    }

    return this;
  }

  /**
   * Removes the specified listener from the listener array for the event
   * named eventName.
   */
  public removeListener(
    eventName: string | symbol,
    listener: GenericFunction,
  ): this {
    if (this._events.has(eventName)) {
      const arr:
        | Array<GenericFunction | WrappedFunction>
        | undefined = this._events.get(eventName);

      assert(arr);

      let listenerIndex = -1;
      for (let i = arr.length - 1; i >= 0; i--) {
        // arr[i]["listener"] is the reference to the listener inside a bound 'once' wrapper
        if (
          arr[i] == listener ||
          (arr[i] && (arr[i] as WrappedFunction)["listener"] == listener)
        ) {
          listenerIndex = i;
          break;
        }
      }

      if (listenerIndex >= 0) {
        arr.splice(listenerIndex, 1);
        this.emit("removeListener", eventName, listener);
        if (arr.length === 0) {
          this._events.delete(eventName);
        }
      }
    }
    return this;
  }

  /**
   * By default EventEmitters will print a warning if more than 10 listeners
   * are added for a particular event. This is a useful default that helps
   * finding memory leaks. Obviously, not all events should be limited to just
   * 10 listeners. The emitter.setMaxListeners() method allows the limit to be
   * modified for this specific EventEmitter instance. The value can be set to
   * Infinity (or 0) to indicate an unlimited number of listeners.
   */
  public setMaxListeners(n: number): this {
    if (n !== Infinity) {
      if (n === 0) {
        n = Infinity;
      } else {
        validateIntegerRange(n, "maxListeners", 0);
      }
    }

    this.maxListeners = n;
    return this;
  }

  /**
   * Creates a Promise that is fulfilled when the EventEmitter emits the given
   * event or that is rejected when the EventEmitter emits 'error'. The Promise
   * will resolve with an array of all the arguments emitted to the given event.
   */
  public static once(
    emitter: EventEmitter | EventTarget,
    name: string,
    // deno-lint-ignore no-explicit-any
  ): Promise<any[]> {
    return new Promise((resolve, reject) => {
      if (emitter instanceof EventTarget) {
        // EventTarget does not have `error` event semantics like Node
        // EventEmitters, we do not listen to `error` events here.
        emitter.addEventListener(
          name,
          (...args) => {
            resolve(args);
          },
          { once: true, passive: false, capture: false },
        );
        return;
      } else if (emitter instanceof EventEmitter) {
        // deno-lint-ignore no-explicit-any
        const eventListener = (...args: any[]): void => {
          if (errorListener !== undefined) {
            emitter.removeListener("error", errorListener);
          }
          resolve(args);
        };
        let errorListener: GenericFunction;

        // Adding an error listener is not optional because
        // if an error is thrown on an event emitter we cannot
        // guarantee that the actual event we are waiting will
        // be fired. The result could be a silent way to create
        // memory or file descriptor leaks, which is something
        // we should avoid.
        if (name !== "error") {
          // deno-lint-ignore no-explicit-any
          errorListener = (err: any): void => {
            emitter.removeListener(name, eventListener);
            reject(err);
          };

          emitter.once("error", errorListener);
        }

        emitter.once(name, eventListener);
        return;
      }
    });
  }

  /**
   * Returns an AsyncIterator that iterates eventName events. It will throw if
   * the EventEmitter emits 'error'. It removes all listeners when exiting the
   * loop. The value returned by each iteration is an array composed of the
   * emitted event arguments.
   */
  public static on(
    emitter: EventEmitter,
    event: string | symbol,
  ): AsyncIterable {
    // deno-lint-ignore no-explicit-any
    const unconsumedEventValues: any[] = [];
    // deno-lint-ignore no-explicit-any
    const unconsumedPromises: any[] = [];
    let error: Error | null = null;
    let finished = false;

    const iterator = {
      // deno-lint-ignore no-explicit-any
      next(): Promise<IteratorResult<any>> {
        // First, we consume all unread events
        // deno-lint-ignore no-explicit-any
        const value: any = unconsumedEventValues.shift();
        if (value) {
          return Promise.resolve(createIterResult(value, false));
        }

        // Then we error, if an error happened
        // This happens one time if at all, because after 'error'
        // we stop listening
        if (error) {
          const p: Promise<never> = Promise.reject(error);
          // Only the first element errors
          error = null;
          return p;
        }

        // If the iterator is finished, resolve to done
        if (finished) {
          return Promise.resolve(createIterResult(undefined, true));
        }

        // Wait until an event happens
        return new Promise(function (resolve, reject) {
          unconsumedPromises.push({ resolve, reject });
        });
      },

      // deno-lint-ignore no-explicit-any
      return(): Promise<IteratorResult<any>> {
        emitter.removeListener(event, eventHandler);
        emitter.removeListener("error", errorHandler);
        finished = true;

        for (const promise of unconsumedPromises) {
          promise.resolve(createIterResult(undefined, true));
        }

        return Promise.resolve(createIterResult(undefined, true));
      },

      throw(err: Error): void {
        error = err;
        emitter.removeListener(event, eventHandler);
        emitter.removeListener("error", errorHandler);
      },

      // deno-lint-ignore no-explicit-any
      [Symbol.asyncIterator](): any {
        return this;
      },
    };

    emitter.on(event, eventHandler);
    emitter.on("error", errorHandler);

    return iterator;

    // deno-lint-ignore no-explicit-any
    function eventHandler(...args: any[]): void {
      const promise = unconsumedPromises.shift();
      if (promise) {
        promise.resolve(createIterResult(args, false));
      } else {
        unconsumedEventValues.push(args);
      }
    }

    // deno-lint-ignore no-explicit-any
    function errorHandler(err: any): void {
      finished = true;

      const toError = unconsumedPromises.shift();
      if (toError) {
        toError.reject(err);
      } else {
        // The next time we call next()
        error = err;
      }

      iterator.return();
    }
  }
}

export { EventEmitter };
export const once = EventEmitter.once;
export const on = EventEmitter.on;
export const captureRejectionSymbol = EventEmitter.captureRejectionSymbol;
export const errorMonitor = EventEmitter.errorMonitor;
