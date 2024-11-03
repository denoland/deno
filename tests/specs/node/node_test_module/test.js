// Copyright Joyent, Inc. and other Node contributors.

// Ported from https://github.com/nodejs/node/blob/d396a041f71cc055ad60b0abc63ad81c0ee6a574/test/fixtures/test-runner/output/output.js

// deno-lint-ignore-file

import assert from "node:assert";
import test from "node:test";
import util from "node:util";
import { setImmediate } from "node:timers";

test("sync pass todo", (t) => {
  t.todo();
});

test("sync pass todo with message", (t) => {
  t.todo("this is a passing todo");
});

test("sync fail todo", (t) => {
  t.todo();
  throw new Error("thrown from sync fail todo");
});

test("sync fail todo with message", (t) => {
  t.todo("this is a failing todo");
  throw new Error("thrown from sync fail todo with message");
});

test("sync skip pass", (t) => {
  t.skip();
});

test("sync skip pass with message", (t) => {
  t.skip("this is skipped");
});

test("sync pass", (t) => {
  t.diagnostic("this test should pass");
});

test("sync throw fail", () => {
  throw new Error("thrown from sync throw fail");
});

test("async skip pass", async (t) => {
  t.skip();
});

test("async pass", async () => {
});

test("async throw fail", async () => {
  throw new Error("thrown from async throw fail");
});

test("nested test", async (t) => {
  await t.test("nested 1", async (t) => {
    await t.test("nested 2", () => {
    });
  });
});

test("async skip fail", async (t) => {
  t.skip();
  throw new Error("thrown from async throw fail");
});

test("async assertion fail", async () => {
  // Make sure the assert module is handled.
  assert.strictEqual(true, false);
});

test("resolve pass", () => {
  return Promise.resolve();
});

test("reject fail", () => {
  return Promise.reject(new Error("rejected from reject fail"));
});

test("unhandled rejection - passes but warns", () => {
  Promise.reject(new Error("rejected from unhandled rejection fail"));
});

test("async unhandled rejection - passes but warns", async () => {
  Promise.reject(new Error("rejected from async unhandled rejection fail"));
});

test("immediate throw - passes but warns", () => {
  setImmediate(() => {
    throw new Error("thrown from immediate throw fail");
  });
});

test("immediate reject - passes but warns", () => {
  setImmediate(() => {
    Promise.reject(new Error("rejected from immediate reject fail"));
  });
});

test("immediate resolve pass", () => {
  return new Promise((resolve) => {
    setImmediate(() => {
      resolve();
    });
  });
});

test("subtest sync throw fail", async (t) => {
  await t.test("+sync throw fail", (t) => {
    t.diagnostic("this subtest should make its parent test fail");
    throw new Error("thrown from subtest sync throw fail");
  });
});

test("sync throw non-error fail", async (t) => {
  throw Symbol("thrown symbol from sync throw non-error fail");
});

test("level 0a", { concurrency: 4 }, async (t) => {
  t.test("level 1a", async (t) => {
    const p1a = new Promise((resolve) => {
      setTimeout(() => {
        resolve();
      }, 100);
    });

    return p1a;
  });

  test("level 1b", async (t) => {
    const p1b = new Promise((resolve) => {
      resolve();
    });

    return p1b;
  });

  t.test("level 1c", async (t) => {
    const p1c = new Promise((resolve) => {
      setTimeout(() => {
        resolve();
      }, 200);
    });

    return p1c;
  });

  t.test("level 1d", async (t) => {
    const p1c = new Promise((resolve) => {
      setTimeout(() => {
        resolve();
      }, 150);
    });

    return p1c;
  });

  const p0a = new Promise((resolve) => {
    setTimeout(() => {
      resolve();
    }, 300);
  });

  return p0a;
});

test("top level", { concurrency: 2 }, async (t) => {
  t.test("+long running", async (t) => {
    return new Promise((resolve, reject) => {
      setTimeout(resolve, 300).unref();
    });
  });

  t.test("+short running", async (t) => {
    t.test("++short running", async (t) => {});
  });
});

test("invalid subtest - pass but subtest fails", (t) => {
  setImmediate(() => {
    t.test("invalid subtest fail", () => {
      throw new Error("this should not be thrown");
    });
  });
});

test("sync skip option", { skip: true }, (t) => {
  throw new Error("this should not be executed");
});

test("sync skip option with message", { skip: "this is skipped" }, (t) => {
  throw new Error("this should not be executed");
});

test("sync skip option is false fail", { skip: false }, (t) => {
  throw new Error("this should be executed");
});

// A test with no arguments provided.
test();

// A test with only a named function provided.
test(function functionOnly() {});

// A test with only an anonymous function provided.
test(() => {});

// A test with only a name provided.
test("test with only a name provided");

// A test with an empty string name.
test("");

// A test with only options provided.
test({ skip: true });

// A test with only a name and options provided.
test("test with a name and options provided", { skip: true });

// A test with only options and a function provided.
test({ skip: true }, function functionAndOptions() {});

// A test whose description needs to be escaped.
// test("escaped description \\ # \\#\\ \n \t \f \v \b \r");

// A test whose skip message needs to be escaped.
test("escaped skip message", { skip: "#skip" });

// A test whose todo message needs to be escaped.
test("escaped todo message", { todo: "#todo" });

// A test with a diagnostic message that needs to be escaped.
test("escaped diagnostic", (t) => {
  t.diagnostic("#diagnostic");
});

test("callback pass", (t, done) => {
  setImmediate(done);
});

test("callback fail", (t, done) => {
  setImmediate(() => {
    done(new Error("callback failure"));
  });
});

test("sync t is this in test", function (t) {
  assert.strictEqual(this, t);
});

test("async t is this in test", async function (t) {
  assert.strictEqual(this, t);
});

test("callback t is this in test", function (t, done) {
  assert.strictEqual(this, t);
  done();
});

test("callback also returns a Promise", async (t, done) => {
  throw new Error("thrown from callback also returns a Promise");
});

test("callback throw", (t, done) => {
  throw new Error("thrown from callback throw");
});

test("callback called twice", (t, done) => {
  done();
  done();
});

test("callback called twice in different ticks", (t, done) => {
  setImmediate(done);
  done();
});

test("callback called twice in future tick", (t, done) => {
  setImmediate(() => {
    done();
    done();
  });
});

test("callback async throw", (t, done) => {
  setImmediate(() => {
    throw new Error("thrown from callback async throw");
  });
});

test("callback async throw after done", (t, done) => {
  setImmediate(() => {
    throw new Error("thrown from callback async throw after done");
  });

  done();
});

test("custom inspect symbol fail", () => {
  const obj = {
    [util.inspect.custom]() {
      return "customized";
    },
    foo: 1,
  };

  throw obj;
});

test("custom inspect symbol that throws fail", () => {
  const obj = {
    [util.inspect.custom]() {
      throw new Error("bad-inspect");
    },
    foo: 1,
  };

  throw obj;
});

test("subtest sync throw fails", async (t) => {
  await t.test("sync throw fails at first", (t) => {
    throw new Error("thrown from subtest sync throw fails at first");
  });
  await t.test("sync throw fails at second", (t) => {
    throw new Error("thrown from subtest sync throw fails at second");
  });
});

test("timed out async test", { timeout: 5 }, async (t) => {
  return new Promise((resolve) => {
    setTimeout(resolve, 100);
  });
});

test("timed out callback test", { timeout: 5 }, (t, done) => {
  setTimeout(done, 100);
});

test("large timeout async test is ok", { timeout: 30_000_000 }, async (t) => {
  return new Promise((resolve) => {
    setTimeout(resolve, 10);
  });
});

test(
  "large timeout callback test is ok",
  { timeout: 30_000_000 },
  (t, done) => {
    setTimeout(done, 10);
  },
);

test("successful thenable", () => {
  let thenCalled = false;
  return {
    get then() {
      if (thenCalled) throw new Error();
      thenCalled = true;
      return (successHandler) => successHandler();
    },
  };
});

test("rejected thenable", () => {
  let thenCalled = false;
  return {
    get then() {
      if (thenCalled) throw new Error();
      thenCalled = true;
      return (_, errorHandler) => errorHandler("custom error");
    },
  };
});

test("unfinished test with uncaughtException", async () => {
  await new Promise(() => {
    setTimeout(() => {
      throw new Error("foo");
    });
  });
});

test("unfinished test with unhandledRejection", async () => {
  await new Promise(() => {
    setTimeout(() => Promise.reject(new Error("bar")));
  });
});
