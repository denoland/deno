// Copyright (c) 2019 Denolibs authors. All rights reserved. MIT license.
// Based on https://github.com/denolibs/event_emitter/blob/master/mod.ts

class EventEmitter {
  public static defaultMaxListeners: number = 10;
  private maxListeners: number | undefined;
  private events: Map<string | symbol, Function[]>;

  public constructor() {
    this.events = new Map();
  }

  private _addListener(
    eventName: string | symbol,
    listener: Function,
    prepend: boolean
  ): this {
    this.emit("newListener", eventName, listener);
    if (this.events.has(eventName)) {
      const listeners = this.events.get(eventName) as Function[];
      if (prepend) {
        listeners.unshift(listener);
      } else {
        listeners.push(listener);
      }
    } else {
      this.events.set(eventName, [listener]);
    }
    const max = this.getMaxListeners();
    if (max > 0 && this.listenerCount(eventName) > max) {
      const warning = new Error(
        `Possible EventEmitter memory leak detected.
         ${this.listenerCount(eventName)} ${eventName.toString()} listeners.
         Use emitter.setMaxListeners() to increase limit`
      );
      warning.name = "MaxListenersExceededWarning";
      console.warn(warning);
    }

    return this;
  }

  public addListener(
    eventName: string | symbol,
    listener: Function
  ): this {
    return this._addListener(eventName, listener, false);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  public emit(eventName: string | symbol, ...args: any[]): boolean {
    if (this.events.has(eventName)) {
      const listeners = (this.events.get(eventName) as Function[]).slice(); // We copy with slice() so array is not mutated during emit
      for (const listener of listeners) {
        try {
          listener.apply(this, args);
        } catch (err) {
          this.emit("error", err);
        }
      }
      return true;
    } else if (eventName === "error") {
      const errMsg = args.length > 0 ? args[0] : Error("Unhandled error.");
      throw errMsg;
    }
    return false;
  }

  public eventNames(): [string | symbol] {
    return Array.from(this.events.keys()) as [string | symbol];
  }

  public getMaxListeners(): number {
    return this.maxListeners || EventEmitter.defaultMaxListeners;
  }

  public listenerCount(eventName: string | symbol): number {
    if (this.events.has(eventName)) {
      return (this.events.get(eventName) as Function[]).length;
    } else {
      return 0;
    }
  }

  private _listeners(
    target: EventEmitter,
    eventName: string | symbol,
    unwrap: boolean
  ): Function[] {
    if (!target.events.has(eventName)) {
      return [];
    }
    const eventListeners: Function[] = target.events.get(
      eventName
    ) as Function[];

    return unwrap
      ? this.unwrapListeners(eventListeners)
      : eventListeners.slice(0);
  }

  private unwrapListeners(arr: Function[]): Function[] {
    let unwrappedListeners: Function[] = new Array(arr.length) as Function[];
    for (let i = 0; i < arr.length; i++) {
      unwrappedListeners[i] = arr[i]["listener"] || arr[i];
    }
    return unwrappedListeners;
  }

  public listeners(eventName: string | symbol): Function[] {
    return this._listeners(this, eventName, true);
  }

  public rawListeners(eventName: string | symbol): Function[] {
    return this._listeners(this, eventName, false);
  }

  public off(eventName: string | symbol, listener: Function): this {
    return this.removeListener(eventName, listener);
  }

  public on(eventName: string | symbol, listener: Function): this {
    return this.addListener(eventName, listener);
  }

  public once(eventName: string | symbol, listener: Function): this {
    const wrapped: Function = this.onceWrap(eventName, listener);
    this.on(eventName, wrapped);
    return this;
  }

  // Wrapped function that calls EventEmitter.removeListener(eventName, self) on execution.
  private onceWrap(eventName: string | symbol, listener: Function): Function {
    const wrapper = function(
      this: {
        eventName: string | symbol;
        listener: Function;
        rawListener: Function;
        context: EventEmitter;
      },
      ...args: any[] // eslint-disable-line @typescript-eslint/no-explicit-any
    ): void {
      this.context.removeListener(this.eventName, this.rawListener);
      this.listener.apply(this.context, args);
    };
    const wrapperContext = {
      eventName: eventName,
      listener: listener,
      rawListener: wrapper,
      context: this
    };
    const wrapped = wrapper.bind(wrapperContext);
    wrapperContext.rawListener = wrapped;
    wrapped.listener = listener;
    return wrapped;
  }

  public prependListener(
    eventName: string | symbol,
    listener: Function
  ): this {
    return this._addListener(eventName, listener, true);
  }

  public prependOnceListener(
    eventName: string | symbol,
    listener: Function
  ): this {
    const wrapped: Function = this.onceWrap(eventName, listener);
    this.prependListener(eventName, wrapped);
    return this;
  }

  public removeAllListeners(eventName?: string | symbol): this {
    if (this.events === undefined) {
      return this;
    }

    if (this.events.has(eventName)) {
      const listeners = (this.events.get(eventName) as Function[]).slice(); // Create a copy; We use it AFTER it's deleted.
      this.events.delete(eventName);
      for (const listener of listeners) {
        this.emit("removeListener", eventName, listener);
      }
    } else {
      const eventList: [string | symbol] = this.eventNames();
      eventList.map((value: string | symbol) => {
        this.removeAllListeners(value)
      });
    }

    return this;
  }

  public removeListener(
    eventName: string | symbol,
    listener: Function
  ): this {
    if (this.events.has(eventName)) {
      const arr: Function[] = this.events.get(eventName) as Function[];
      if (arr.indexOf(listener) !== -1) {
        arr.splice(arr.indexOf(listener), 1);
        this.emit("removeListener", eventName, listener);
        if (arr.length === 0) {
          this.events.delete(eventName);
        }
      }
    }
    return this;
  }

  public setMaxListeners(n: number): this {
    this.maxListeners = n;
    return this;
  }
}

export default EventEmitter;