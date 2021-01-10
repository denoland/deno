// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  fail,
} from "../testing/asserts.ts";
import EventEmitter, { on, once, WrappedFunction } from "./events.ts";

const shouldNeverBeEmitted = () => {
  fail("Should never be called");
};

Deno.test({
  name:
    'When adding a new event, "eventListener" event is fired before adding the listener',
  fn() {
    let eventsFired: string[] = [];
    const testEmitter = new EventEmitter();
    testEmitter.on("newListener", (event: string) => {
      if (event !== "newListener") {
        eventsFired.push("newListener");
      }
    });
    testEmitter.on("event", () => {
      eventsFired.push("event");
    });
    assertEquals(eventsFired, ["newListener"]);
    eventsFired = [];
    testEmitter.emit("event");
    assertEquals(eventsFired, ["event"]);
  },
});

Deno.test({
  name:
    'When removing a listenert, "removeListener" event is fired after removal',
  fn() {
    const eventsFired: string[] = [];
    const testEmitter = new EventEmitter();
    testEmitter.on("removeListener", () => {
      eventsFired.push("removeListener");
    });
    const eventFunction = function (): void {
      eventsFired.push("event");
    };
    testEmitter.on("event", eventFunction);

    assertEquals(eventsFired, []);
    testEmitter.removeListener("event", eventFunction);
    assertEquals(eventsFired, ["removeListener"]);
  },
});

Deno.test({
  name:
    "Default max listeners is 10, but can be changed by direct assignment only",
  fn() {
    assertEquals(EventEmitter.defaultMaxListeners, 10);
    new EventEmitter().setMaxListeners(20);
    assertEquals(EventEmitter.defaultMaxListeners, 10);
    EventEmitter.defaultMaxListeners = 20;
    assertEquals(EventEmitter.defaultMaxListeners, 20);
    EventEmitter.defaultMaxListeners = 10; //reset back to original value

    assertThrows(() => {
      new EventEmitter().setMaxListeners(-1);
    });

    const ee = new EventEmitter();
    const noop = (): void => {};
    const origWarn = console.warn;

    for (let i = 10; i--;) {
      ee.on("test", noop);
    }

    // there are only sync actions until it gets restored,
    // so it's safe to overwrite this
    console.warn = (): void => fail("Infinity listeners should be allowed");

    ee.setMaxListeners(Infinity);
    ee.on("test", noop);

    // 0 means that unlimited listeners are allowed
    ee.setMaxListeners(0);
    ee.on("test", noop);

    console.warn = origWarn;
  },
});

Deno.test({
  name: "addListener adds a listener, and listener count is correct",
  fn() {
    const testEmitter = new EventEmitter();
    testEmitter.on("event", shouldNeverBeEmitted);
    assertEquals(1, testEmitter.listenerCount("event"));
    testEmitter.on("event", shouldNeverBeEmitted);
    assertEquals(2, testEmitter.listenerCount("event"));
  },
});

Deno.test({
  name: "Emitted events are called synchronously in the order they were added",
  fn() {
    const testEmitter = new EventEmitter();
    const eventsFired: string[] = [];
    testEmitter.on("event", (oneArg: string) => {
      eventsFired.push("event(" + oneArg + ")");
    });
    testEmitter.on("event", (oneArg: string, twoArg: string) => {
      eventsFired.push("event(" + oneArg + ", " + twoArg + ")");
    });

    testEmitter.on("non-event", shouldNeverBeEmitted);

    testEmitter.on(
      "event",
      (oneArg: string, twoArg: string, threeArg: string) => {
        eventsFired.push(
          "event(" + oneArg + ", " + twoArg + ", " + threeArg + ")",
        );
      },
    );
    testEmitter.emit("event", 1, 2, 3);
    assertEquals(eventsFired, ["event(1)", "event(1, 2)", "event(1, 2, 3)"]);
  },
});

Deno.test({
  name: "Registered event names are returned as strings or Sybols",
  fn() {
    const testEmitter = new EventEmitter();
    testEmitter.on("event", shouldNeverBeEmitted);
    testEmitter.on("event", shouldNeverBeEmitted);
    const sym = Symbol("symbol");
    testEmitter.on(sym, shouldNeverBeEmitted);
    assertEquals(testEmitter.eventNames(), ["event", sym]);
  },
});

Deno.test({
  name: "You can set and get max listeners",
  fn() {
    const testEmitter = new EventEmitter();
    assertEquals(testEmitter.getMaxListeners(), 10);
    testEmitter.setMaxListeners(20);
    assertEquals(testEmitter.getMaxListeners(), 20);
  },
});

Deno.test({
  name: "You can retrieve registered functions for an event",
  fn() {
    const testEmitter = new EventEmitter();
    testEmitter.on("someOtherEvent", shouldNeverBeEmitted);
    testEmitter.on("event", shouldNeverBeEmitted);
    const testFunction = (): void => {};
    testEmitter.on("event", testFunction);
    assertEquals(testEmitter.listeners("event"), [
      shouldNeverBeEmitted,
      testFunction,
    ]);
  },
});

Deno.test({
  name: "Off is alias for removeListener",
  fn() {
    const testEmitter = new EventEmitter();
    testEmitter.on("event", shouldNeverBeEmitted);
    assertEquals(testEmitter.listenerCount("event"), 1);
    testEmitter.off("event", shouldNeverBeEmitted);
    assertEquals(testEmitter.listenerCount("event"), 0);
  },
});

Deno.test({
  name: "Event registration can be chained",
  fn() {
    const testEmitter = new EventEmitter();
    testEmitter
      .on("event", shouldNeverBeEmitted)
      .on("event", shouldNeverBeEmitted);
    assertEquals(testEmitter.listenerCount("event"), 2);
  },
});

Deno.test({
  name: "Events can be registered to only fire once",
  fn() {
    let eventsFired: string[] = [];
    const testEmitter = new EventEmitter();
    //prove multiple emits on same event first (when registered with 'on')
    testEmitter.on("multiple event", () => {
      eventsFired.push("multiple event");
    });
    testEmitter.emit("multiple event");
    testEmitter.emit("multiple event");
    assertEquals(eventsFired, ["multiple event", "multiple event"]);

    //now prove multiple events registered via 'once' only emit once
    eventsFired = [];
    testEmitter.once("single event", () => {
      eventsFired.push("single event");
    });
    testEmitter.emit("single event");
    testEmitter.emit("single event");
    assertEquals(eventsFired, ["single event"]);
  },
});

Deno.test({
  name:
    "You can inject a listener into the start of the stack, rather than at the end",
  fn() {
    const eventsFired: string[] = [];
    const testEmitter = new EventEmitter();
    testEmitter.on("event", () => {
      eventsFired.push("first");
    });
    testEmitter.on("event", () => {
      eventsFired.push("second");
    });
    testEmitter.prependListener("event", () => {
      eventsFired.push("third");
    });
    testEmitter.emit("event");
    assertEquals(eventsFired, ["third", "first", "second"]);
  },
});

Deno.test({
  name: 'You can prepend a "once" listener',
  fn() {
    const eventsFired: string[] = [];
    const testEmitter = new EventEmitter();
    testEmitter.on("event", () => {
      eventsFired.push("first");
    });
    testEmitter.on("event", () => {
      eventsFired.push("second");
    });
    testEmitter.prependOnceListener("event", () => {
      eventsFired.push("third");
    });
    testEmitter.emit("event");
    testEmitter.emit("event");
    assertEquals(eventsFired, ["third", "first", "second", "first", "second"]);
  },
});

Deno.test({
  name: "Remove all listeners, which can also be chained",
  fn() {
    const testEmitter = new EventEmitter();
    testEmitter.on("event", shouldNeverBeEmitted);
    testEmitter.on("event", shouldNeverBeEmitted);
    testEmitter.on("other event", shouldNeverBeEmitted);
    testEmitter.on("other event", shouldNeverBeEmitted);
    testEmitter.once("other event", shouldNeverBeEmitted);
    assertEquals(testEmitter.listenerCount("event"), 2);
    assertEquals(testEmitter.listenerCount("other event"), 3);

    testEmitter.removeAllListeners("event").removeAllListeners("other event");

    assertEquals(testEmitter.listenerCount("event"), 0);
    assertEquals(testEmitter.listenerCount("other event"), 0);
  },
});

Deno.test({
  name: "Provide a non-existent event to removeAllListeners will do nothing",
  fn() {
    const testEmitter = new EventEmitter();
    testEmitter.on("event", shouldNeverBeEmitted);
    testEmitter.on("event", shouldNeverBeEmitted);
    testEmitter.on("other event", shouldNeverBeEmitted);
    testEmitter.on("other event", shouldNeverBeEmitted);
    testEmitter.once("other event", shouldNeverBeEmitted);
    assertEquals(testEmitter.listenerCount("event"), 2);
    assertEquals(testEmitter.listenerCount("other event"), 3);

    testEmitter.removeAllListeners("non-existent");

    assertEquals(testEmitter.listenerCount("event"), 2);
    assertEquals(testEmitter.listenerCount("other event"), 3);
  },
});

Deno.test({
  name: "Remove individual listeners, which can also be chained",
  fn() {
    const testEmitter = new EventEmitter();
    testEmitter.on("event", shouldNeverBeEmitted);
    testEmitter.on("event", shouldNeverBeEmitted);
    testEmitter.once("other event", shouldNeverBeEmitted);
    assertEquals(testEmitter.listenerCount("event"), 2);
    assertEquals(testEmitter.listenerCount("other event"), 1);

    testEmitter.removeListener("other event", shouldNeverBeEmitted);
    assertEquals(testEmitter.listenerCount("event"), 2);
    assertEquals(testEmitter.listenerCount("other event"), 0);

    testEmitter
      .removeListener("event", shouldNeverBeEmitted)
      .removeListener("event", shouldNeverBeEmitted);

    assertEquals(testEmitter.listenerCount("event"), 0);
    assertEquals(testEmitter.listenerCount("other event"), 0);
  },
});

Deno.test({
  name: "It is OK to try to remove non-existent listener",
  fn() {
    const testEmitter = new EventEmitter();

    const madeUpEvent = (): void => {
      fail("Should never be called");
    };

    testEmitter.on("event", shouldNeverBeEmitted);
    assertEquals(testEmitter.listenerCount("event"), 1);

    testEmitter.removeListener("event", madeUpEvent);
    testEmitter.removeListener("non-existent event", madeUpEvent);

    assertEquals(testEmitter.listenerCount("event"), 1);
  },
});

Deno.test({
  name: "all listeners complete execution even if removed before execution",
  fn() {
    const testEmitter = new EventEmitter();
    let eventsProcessed: string[] = [];
    const listenerB = (): number => eventsProcessed.push("B");
    const listenerA = (): void => {
      eventsProcessed.push("A");
      testEmitter.removeListener("event", listenerB);
    };

    testEmitter.on("event", listenerA);
    testEmitter.on("event", listenerB);

    testEmitter.emit("event");
    assertEquals(eventsProcessed, ["A", "B"]);

    eventsProcessed = [];
    testEmitter.emit("event");
    assertEquals(eventsProcessed, ["A"]);
  },
});

Deno.test({
  name: 'Raw listener will return event listener or wrapped "once" function',
  fn() {
    const testEmitter = new EventEmitter();
    const eventsProcessed: string[] = [];
    const listenerA = (): number => eventsProcessed.push("A");
    const listenerB = (): number => eventsProcessed.push("B");
    testEmitter.on("event", listenerA);
    testEmitter.once("once-event", listenerB);

    const rawListenersForEvent = testEmitter.rawListeners("event");
    const rawListenersForOnceEvent = testEmitter.rawListeners("once-event");

    assertEquals(rawListenersForEvent.length, 1);
    assertEquals(rawListenersForOnceEvent.length, 1);
    assertEquals(rawListenersForEvent[0], listenerA);
    assertEquals(
      (rawListenersForOnceEvent[0] as WrappedFunction).listener,
      listenerB,
    );
  },
});

Deno.test({
  name:
    "Once wrapped raw listeners may be executed multiple times, until the wrapper is executed",
  fn() {
    const testEmitter = new EventEmitter();
    let eventsProcessed: string[] = [];
    const listenerA = (): number => eventsProcessed.push("A");
    testEmitter.once("once-event", listenerA);

    const rawListenersForOnceEvent = testEmitter.rawListeners("once-event");
    const wrappedFn: WrappedFunction =
      rawListenersForOnceEvent[0] as WrappedFunction;
    wrappedFn.listener();
    wrappedFn.listener();
    wrappedFn.listener();
    assertEquals(eventsProcessed, ["A", "A", "A"]);

    eventsProcessed = [];
    wrappedFn(); // executing the wrapped listener function will remove it from the event
    assertEquals(eventsProcessed, ["A"]);
    assertEquals(testEmitter.listeners("once-event").length, 0);
  },
});

Deno.test({
  name: "Can add once event listener to EventEmitter via standalone function",
  async fn() {
    const ee = new EventEmitter();
    setTimeout(() => {
      ee.emit("event", 42, "foo");
    }, 0);
    // deno-lint-ignore no-explicit-any
    const valueArr: any[] = await once(ee, "event");
    assertEquals(valueArr, [42, "foo"]);
  },
});

Deno.test({
  name: "Can add once event listener to EventTarget via standalone function",
  async fn() {
    const et: EventTarget = new EventTarget();
    setTimeout(() => {
      const event: Event = new Event("event", { composed: true });
      et.dispatchEvent(event);
    }, 0);
    // deno-lint-ignore no-explicit-any
    const eventObj: any[] = await once(et, "event");
    assert(!eventObj[0].isTrusted);
  },
});

Deno.test({
  name: "Only valid integers are allowed for max listeners",
  fn() {
    const ee = new EventEmitter();
    ee.setMaxListeners(0);
    assertThrows(
      () => {
        ee.setMaxListeners(-1);
      },
      Error,
      "must be >= 0",
    );
    assertThrows(
      () => {
        ee.setMaxListeners(3.45);
      },
      Error,
      "must be 'an integer'",
    );
  },
});

Deno.test({
  name: "ErrorMonitor can spy on error events without consuming them",
  fn() {
    const ee = new EventEmitter();
    let events: string[] = [];
    //unhandled error scenario should throw
    assertThrows(
      () => {
        ee.emit("error");
      },
      Error,
      "Unhandled error",
    );

    ee.on(EventEmitter.errorMonitor, () => {
      events.push("errorMonitor event");
    });

    //error is still unhandled but also intercepted by error monitor
    assertThrows(
      () => {
        ee.emit("error");
      },
      Error,
      "Unhandled error",
    );
    assertEquals(events, ["errorMonitor event"]);

    //A registered error handler won't throw, but still be monitored
    events = [];
    ee.on("error", () => {
      events.push("error");
    });
    ee.emit("error");
    assertEquals(events, ["errorMonitor event", "error"]);
  },
});

Deno.test({
  name: "asynchronous iteration of events are handled as expected",
  async fn() {
    const ee = new EventEmitter();
    setTimeout(() => {
      ee.emit("foo", "bar");
      ee.emit("bar", 24);
      ee.emit("foo", 42);
    }, 0);

    const iterable = on(ee, "foo");

    const expected = [["bar"], [42]];

    for await (const event of iterable) {
      const current = expected.shift();

      assertEquals(current, event);

      if (expected.length === 0) {
        break;
      }
    }
    assertEquals(ee.listenerCount("foo"), 0);
    assertEquals(ee.listenerCount("error"), 0);
  },
});

Deno.test({
  name: "asynchronous error handling of emitted events works as expected",
  async fn() {
    const ee = new EventEmitter();
    const _err = new Error("kaboom");
    setTimeout(() => {
      ee.emit("error", _err);
    }, 0);

    const iterable = on(ee, "foo");
    let thrown = false;

    try {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      for await (const event of iterable) {
        fail("no events should be processed due to the error thrown");
      }
    } catch (err) {
      thrown = true;
      assertEquals(err, _err);
    }
    assertEquals(thrown, true);
  },
});

Deno.test({
  name: "error thrown during asynchronous processing of events is handled",
  async fn() {
    const ee = new EventEmitter();
    const _err = new Error("kaboom");
    setTimeout(() => {
      ee.emit("foo", 42);
      ee.emit("error", _err);
    }, 0);

    const iterable = on(ee, "foo");
    const expected = [[42]];
    let thrown = false;

    try {
      for await (const event of iterable) {
        const current = expected.shift();
        assertEquals(current, event);
      }
    } catch (err) {
      thrown = true;
      assertEquals(err, _err);
    }
    assertEquals(thrown, true);
    assertEquals(ee.listenerCount("foo"), 0);
    assertEquals(ee.listenerCount("error"), 0);
  },
});

Deno.test({
  name:
    "error thrown in processing loop of asynchronous event prevents processing of additional events",
  async fn() {
    const ee = new EventEmitter();
    const _err = new Error("kaboom");

    setTimeout(() => {
      ee.emit("foo", 42);
      ee.emit("foo", 999);
    }, 0);

    try {
      for await (const event of on(ee, "foo")) {
        assertEquals(event, [42]);
        throw _err;
      }
    } catch (err) {
      assertEquals(err, _err);
    }

    assertEquals(ee.listenerCount("foo"), 0);
    assertEquals(ee.listenerCount("error"), 0);
  },
});

Deno.test({
  name: "asynchronous iterator next() works as expected",
  async fn() {
    const ee = new EventEmitter();
    const iterable = on(ee, "foo");

    setTimeout(function () {
      ee.emit("foo", "bar");
      ee.emit("foo", 42);
      iterable.return();
    }, 0);

    const results = await Promise.all([
      iterable.next(),
      iterable.next(),
      iterable.next(),
    ]);

    assertEquals(results, [
      {
        value: ["bar"],
        done: false,
      },
      {
        value: [42],
        done: false,
      },
      {
        value: undefined,
        done: true,
      },
    ]);

    assertEquals(await iterable.next(), {
      value: undefined,
      done: true,
    });
  },
});

Deno.test({
  name: "async iterable throw handles various scenarios",
  async fn() {
    const ee = new EventEmitter();
    const iterable = on(ee, "foo");

    setTimeout(() => {
      ee.emit("foo", "bar");
      ee.emit("foo", 42); // lost in the queue
      iterable.throw(_err);
    }, 0);

    const _err = new Error("kaboom");
    let thrown = false;

    const expected = [["bar"], [42]];

    try {
      for await (const event of iterable) {
        assertEquals(event, expected.shift());
      }
    } catch (err) {
      thrown = true;
      assertEquals(err, _err);
    }
    assertEquals(thrown, true);
    assertEquals(expected.length, 0);
    assertEquals(ee.listenerCount("foo"), 0);
    assertEquals(ee.listenerCount("error"), 0);
  },
});

// Event emitter's `on` previously referenced addListener internally, so overriding addListener
// would cause a deadlock
// This is a regression test
Deno.test("Elements that extend EventEmitter listener alias don't end up in a deadlock", () => {
  class X extends EventEmitter {
    addListener(eventName: string, listener: () => void) {
      return super.on(eventName, listener);
    }
  }

  const x = new X();
  try {
    x.on("x", () => {});
  } catch (e) {
    fail();
  }
});
