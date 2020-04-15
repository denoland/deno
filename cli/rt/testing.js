System.register(
  "$deno$/testing.ts",
  [
    "$deno$/colors.ts",
    "$deno$/ops/os.ts",
    "$deno$/web/console.ts",
    "$deno$/files.ts",
    "$deno$/internals.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/ops/runtime.ts",
    "$deno$/ops/resources.ts",
    "$deno$/util.ts",
  ],
  function (exports_69, context_69) {
    "use strict";
    let colors_ts_1,
      os_ts_1,
      console_ts_1,
      files_ts_5,
      internals_ts_3,
      text_encoding_ts_5,
      runtime_ts_4,
      resources_ts_5,
      util_ts_8,
      RED_FAILED,
      GREEN_OK,
      YELLOW_IGNORED,
      disabledConsole,
      TEST_REGISTRY,
      encoder,
      TestApi;
    const __moduleName = context_69 && context_69.id;
    function delay(n) {
      return new Promise((resolve, _) => {
        setTimeout(resolve, n);
      });
    }
    function formatDuration(time = 0) {
      const timeStr = `(${time}ms)`;
      return colors_ts_1.gray(colors_ts_1.italic(timeStr));
    }
    // Wrap test function in additional assertion that makes sure
    // the test case does not leak async "ops" - ie. number of async
    // completed ops after the test is the same as number of dispatched
    // ops. Note that "unref" ops are ignored since in nature that are
    // optional.
    function assertOps(fn) {
      return async function asyncOpSanitizer() {
        const pre = runtime_ts_4.metrics();
        await fn();
        // Defer until next event loop turn - that way timeouts and intervals
        // cleared can actually be removed from resource table, otherwise
        // false positives may occur (https://github.com/denoland/deno/issues/4591)
        await delay(0);
        const post = runtime_ts_4.metrics();
        // We're checking diff because one might spawn HTTP server in the background
        // that will be a pending async op before test starts.
        const dispatchedDiff = post.opsDispatchedAsync - pre.opsDispatchedAsync;
        const completedDiff = post.opsCompletedAsync - pre.opsCompletedAsync;
        util_ts_8.assert(
          dispatchedDiff === completedDiff,
          `Test case is leaking async ops.
Before:
  - dispatched: ${pre.opsDispatchedAsync}
  - completed: ${pre.opsCompletedAsync}
After:
  - dispatched: ${post.opsDispatchedAsync}
  - completed: ${post.opsCompletedAsync}`
        );
      };
    }
    // Wrap test function in additional assertion that makes sure
    // the test case does not "leak" resources - ie. resource table after
    // the test has exactly the same contents as before the test.
    function assertResources(fn) {
      return async function resourceSanitizer() {
        const pre = resources_ts_5.resources();
        await fn();
        const post = resources_ts_5.resources();
        const preStr = JSON.stringify(pre, null, 2);
        const postStr = JSON.stringify(post, null, 2);
        const msg = `Test case is leaking resources.
Before: ${preStr}
After: ${postStr}`;
        util_ts_8.assert(preStr === postStr, msg);
      };
    }
    // Main test function provided by Deno, as you can see it merely
    // creates a new object with "name" and "fn" fields.
    function test(t, fn) {
      let testDef;
      if (typeof t === "string") {
        if (!fn || typeof fn != "function") {
          throw new TypeError("Missing test function");
        }
        if (!t) {
          throw new TypeError("The test name can't be empty");
        }
        testDef = { fn: fn, name: t, ignore: false };
      } else if (typeof t === "function") {
        if (!t.name) {
          throw new TypeError("The test function can't be anonymous");
        }
        testDef = { fn: t, name: t.name, ignore: false };
      } else {
        if (!t.fn) {
          throw new TypeError("Missing test function");
        }
        if (!t.name) {
          throw new TypeError("The test name can't be empty");
        }
        testDef = { ...t, ignore: Boolean(t.ignore) };
      }
      if (testDef.disableOpSanitizer !== true) {
        testDef.fn = assertOps(testDef.fn);
      }
      if (testDef.disableResourceSanitizer !== true) {
        testDef.fn = assertResources(testDef.fn);
      }
      TEST_REGISTRY.push(testDef);
    }
    exports_69("test", test);
    function log(msg, noNewLine = false) {
      if (!noNewLine) {
        msg += "\n";
      }
      // Using `stdout` here because it doesn't force new lines
      // compared to `console.log`; `core.print` on the other hand
      // is line-buffered and doesn't output message without newline
      files_ts_5.stdout.writeSync(encoder.encode(msg));
    }
    function reportToConsole(message) {
      if (message.start != null) {
        log(`running ${message.start.tests.length} tests`);
      } else if (message.testStart != null) {
        const { name } = message.testStart;
        log(`test ${name} ... `, true);
        return;
      } else if (message.testEnd != null) {
        switch (message.testEnd.status) {
          case "passed":
            log(`${GREEN_OK} ${formatDuration(message.testEnd.duration)}`);
            break;
          case "failed":
            log(`${RED_FAILED} ${formatDuration(message.testEnd.duration)}`);
            break;
          case "ignored":
            log(
              `${YELLOW_IGNORED} ${formatDuration(message.testEnd.duration)}`
            );
            break;
        }
      } else if (message.end != null) {
        const failures = message.end.results.filter((m) => m.error != null);
        if (failures.length > 0) {
          log(`\nfailures:\n`);
          for (const { name, error } of failures) {
            log(name);
            log(console_ts_1.stringifyArgs([error]));
            log("");
          }
          log(`failures:\n`);
          for (const { name } of failures) {
            log(`\t${name}`);
          }
        }
        log(
          `\ntest result: ${message.end.failed ? RED_FAILED : GREEN_OK}. ` +
            `${message.end.passed} passed; ${message.end.failed} failed; ` +
            `${message.end.ignored} ignored; ${message.end.measured} measured; ` +
            `${message.end.filtered} filtered out ` +
            `${formatDuration(message.end.duration)}\n`
        );
      }
    }
    function createFilterFn(filter, skip) {
      return (def) => {
        let passes = true;
        if (filter) {
          if (filter instanceof RegExp) {
            passes = passes && filter.test(def.name);
          } else {
            passes = passes && def.name.includes(filter);
          }
        }
        if (skip) {
          if (skip instanceof RegExp) {
            passes = passes && !skip.test(def.name);
          } else {
            passes = passes && !def.name.includes(skip);
          }
        }
        return passes;
      };
    }
    async function runTests({
      exitOnFail = true,
      failFast = false,
      filter = undefined,
      skip = undefined,
      disableLog = false,
      reportToConsole: reportToConsole_ = true,
      onMessage = undefined,
    } = {}) {
      const filterFn = createFilterFn(filter, skip);
      const testApi = new TestApi(TEST_REGISTRY, filterFn, failFast);
      // @ts-ignore
      const originalConsole = globalThis.console;
      if (disableLog) {
        // @ts-ignore
        globalThis.console = disabledConsole;
      }
      let endMsg;
      for await (const message of testApi) {
        if (onMessage != null) {
          await onMessage(message);
        }
        if (reportToConsole_) {
          reportToConsole(message);
        }
        if (message.end != null) {
          endMsg = message.end;
        }
      }
      if (disableLog) {
        // @ts-ignore
        globalThis.console = originalConsole;
      }
      if (endMsg.failed > 0 && exitOnFail) {
        os_ts_1.exit(1);
      }
      return endMsg;
    }
    exports_69("runTests", runTests);
    return {
      setters: [
        function (colors_ts_1_1) {
          colors_ts_1 = colors_ts_1_1;
        },
        function (os_ts_1_1) {
          os_ts_1 = os_ts_1_1;
        },
        function (console_ts_1_1) {
          console_ts_1 = console_ts_1_1;
        },
        function (files_ts_5_1) {
          files_ts_5 = files_ts_5_1;
        },
        function (internals_ts_3_1) {
          internals_ts_3 = internals_ts_3_1;
        },
        function (text_encoding_ts_5_1) {
          text_encoding_ts_5 = text_encoding_ts_5_1;
        },
        function (runtime_ts_4_1) {
          runtime_ts_4 = runtime_ts_4_1;
        },
        function (resources_ts_5_1) {
          resources_ts_5 = resources_ts_5_1;
        },
        function (util_ts_8_1) {
          util_ts_8 = util_ts_8_1;
        },
      ],
      execute: function () {
        RED_FAILED = colors_ts_1.red("FAILED");
        GREEN_OK = colors_ts_1.green("ok");
        YELLOW_IGNORED = colors_ts_1.yellow("ignored");
        disabledConsole = new console_ts_1.Console(() => {});
        TEST_REGISTRY = [];
        encoder = new text_encoding_ts_5.TextEncoder();
        internals_ts_3.exposeForTest("reportToConsole", reportToConsole);
        // TODO: already implements AsyncGenerator<RunTestsMessage>, but add as "implements to class"
        // TODO: implements PromiseLike<RunTestsEndResult>
        TestApi = class TestApi {
          constructor(tests, filterFn, failFast) {
            this.tests = tests;
            this.filterFn = filterFn;
            this.failFast = failFast;
            this.stats = {
              filtered: 0,
              ignored: 0,
              measured: 0,
              passed: 0,
              failed: 0,
            };
            this.testsToRun = tests.filter(filterFn);
            this.stats.filtered = tests.length - this.testsToRun.length;
          }
          async *[Symbol.asyncIterator]() {
            yield { start: { tests: this.testsToRun } };
            const results = [];
            const suiteStart = +new Date();
            for (const test of this.testsToRun) {
              const endMessage = {
                name: test.name,
                duration: 0,
              };
              yield { testStart: { ...test } };
              if (test.ignore) {
                endMessage.status = "ignored";
                this.stats.ignored++;
              } else {
                const start = +new Date();
                try {
                  await test.fn();
                  endMessage.status = "passed";
                  this.stats.passed++;
                } catch (err) {
                  endMessage.status = "failed";
                  endMessage.error = err;
                  this.stats.failed++;
                }
                endMessage.duration = +new Date() - start;
              }
              results.push(endMessage);
              yield { testEnd: endMessage };
              if (this.failFast && endMessage.error != null) {
                break;
              }
            }
            const duration = +new Date() - suiteStart;
            yield { end: { ...this.stats, duration, results } };
          }
        };
      },
    };
  }
);
