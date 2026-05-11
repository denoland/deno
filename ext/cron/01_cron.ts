// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, internals, primordials } = globalThis.__bootstrap;
const {
  op_cron_create,
  op_cron_next,
  op_cron_persistent_create,
  op_cron_persistent_list,
  op_cron_persistent_remove,
} = core.ops;
const {
  ArrayIsArray,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypeSort,
  NumberPrototypeToString,
  ObjectKeys,
  ObjectPrototypeHasOwnProperty,
  PromiseResolve,
  SafeArrayIterator,
  TypeError,
} = primordials;
const {
  otelState,
  builtinTracer,
  ContextManager,
  enterSpan,
  restoreSnapshot,
} = core.loadExtScript("ext:deno_telemetry/telemetry.ts");
const { updateSpanFromError } = core.loadExtScript(
  "ext:deno_telemetry/util.ts",
);

function formatToCronSchedule(
  value?: number | { exact: number | number[] } | {
    start?: number;
    end?: number;
    every?: number;
  },
): string {
  if (value === undefined) {
    return "*";
  } else if (typeof value === "number") {
    return NumberPrototypeToString(value);
  } else {
    const { exact } = value as { exact: number | number[] };
    if (exact === undefined) {
      const { start, end, every } = value as {
        start?: number;
        end?: number;
        every?: number;
      };
      if (start !== undefined && end !== undefined && every !== undefined) {
        return start + "-" + end + "/" + every;
      } else if (start !== undefined && end !== undefined) {
        return start + "-" + end;
      } else if (start !== undefined && every !== undefined) {
        return start + "/" + every;
      } else if (start !== undefined) {
        return start + "/1";
      } else if (end === undefined && every !== undefined) {
        return "*/" + every;
      } else {
        throw new TypeError(
          `Invalid cron schedule: start=${start}, end=${end}, every=${every}`,
        );
      }
    } else {
      if (typeof exact === "number") {
        return NumberPrototypeToString(exact);
      } else {
        return ArrayPrototypeJoin(exact, ",");
      }
    }
  }
}

function parseScheduleToString(
  schedule: string | Deno.CronSchedule,
): string {
  if (typeof schedule === "string") {
    return schedule;
  } else {
    let {
      minute,
      hour,
      dayOfMonth,
      month,
      dayOfWeek,
    } = schedule;

    // Automatically override unspecified values for convenience. For example,
    // to run every 2 hours, `{ hour: { every: 2 } }` can be specified without
    // explicitly specifying `minute`.
    if (minute !== undefined) {
      // Nothing to override.
    } else if (hour !== undefined) {
      // Override minute to 0 since it's not specified.
      minute = 0;
    } else if (dayOfMonth !== undefined || dayOfWeek !== undefined) {
      // Override minute and hour to 0 since they're not specified.
      minute = 0;
      hour = 0;
    } else if (month !== undefined) {
      // Override minute and hour to 0, and dayOfMonth to 1 since they're not specified.
      minute = 0;
      hour = 0;
      dayOfMonth = 1;
    }

    return formatToCronSchedule(minute) +
      " " + formatToCronSchedule(hour) +
      " " + formatToCronSchedule(dayOfMonth) +
      " " + formatToCronSchedule(month) +
      " " + formatToCronSchedule(dayOfWeek);
  }
}

function cron(
  name: string,
  schedule: string | Deno.CronSchedule,
  handlerOrOptions1:
    | (() => Promise<void> | void)
    | ({ backoffSchedule?: number[]; signal?: AbortSignal }),
  handler2?: () => Promise<void> | void,
) {
  if (name === undefined) {
    throw new TypeError(
      "Cannot create cron job, a unique name is required: received 'undefined'",
    );
  }
  if (schedule === undefined) {
    throw new TypeError(
      "Cannot create cron job, a schedule is required: received 'undefined'",
    );
  }

  schedule = parseScheduleToString(schedule);

  let handler: () => Promise<void> | void;
  let options:
    | { backoffSchedule?: number[]; signal?: AbortSignal }
    | undefined = undefined;

  if (typeof handlerOrOptions1 === "function") {
    handler = handlerOrOptions1;
    if (handler2 !== undefined) {
      throw new TypeError(
        "Cannot create cron job, a single handler is required: two handlers were specified",
      );
    }
  } else if (typeof handler2 === "function") {
    handler = handler2;
    options = handlerOrOptions1;
  } else {
    throw new TypeError("Cannot create cron job: a handler is required");
  }

  const rid = op_cron_create(
    name,
    schedule,
    options?.backoffSchedule,
  );

  if (options?.signal) {
    const signal = options?.signal;
    signal.addEventListener(
      "abort",
      () => {
        core.close(rid);
      },
      { once: true },
    );
  }

  return (async () => {
    let success = true;
    while (true) {
      const r = await op_cron_next(rid, success);
      if (!r.active) {
        break;
      }
      let span;
      if (otelState.TRACING_ENABLED) {
        let activeContext = ContextManager.active();
        if (r.traceparent) {
          for (
            const propagator of new SafeArrayIterator(otelState.PROPAGATORS)
          ) {
            activeContext = propagator.extract(activeContext, {}, {
              get(_carrier, key) {
                if (key === "traceparent") return r.traceparent;
              },
              keys(_carrier) {
                return ["traceparent"];
              },
            });
          }
        }

        span = builtinTracer().startSpan(
          "deno.cron",
          { kind: 0 },
          activeContext,
        );
        span.setAttribute("deno.cron.name", name);
        span.setAttribute("deno.cron.schedule", schedule);
      }
      try {
        if (span) {
          const snapshot = enterSpan(span);
          let result;
          try {
            result = handler();
          } finally {
            if (snapshot) restoreSnapshot(snapshot);
          }
          await result;
          span.setStatus({ code: 1 });
          span.end();
        } else {
          await handler();
        }
        success = true;
      } catch (error) {
        if (span) {
          updateSpanFromError(span, error);
          span.end();
        }
        internals.log("error", `Exception in cron handler ${name}`, error);
        success = false;
      }
    }
  })();
}

function persistent(
  options: {
    name: string;
    schedule: string | Deno.CronSchedule;
    script: string;
    permissions?: string[];
    cwd?: string;
    env?: Record<string, string>;
  },
): Promise<void> {
  if (options === null || typeof options !== "object") {
    throw new TypeError(
      "Cannot register persistent cron: options must be an object",
    );
  }
  const { name, schedule, script } = options;
  if (typeof name !== "string" || name.length === 0) {
    throw new TypeError(
      "Cannot register persistent cron: 'name' must be a non-empty string",
    );
  }
  if (schedule === undefined) {
    throw new TypeError(
      "Cannot register persistent cron: 'schedule' is required",
    );
  }
  if (typeof script !== "string" || script.length === 0) {
    throw new TypeError(
      "Cannot register persistent cron: 'script' must be a non-empty path",
    );
  }

  const scheduleStr = parseScheduleToString(schedule);

  const permissions = options.permissions ?? [];
  if (!ArrayIsArray(permissions)) {
    throw new TypeError(
      "Cannot register persistent cron: 'permissions' must be an array of strings",
    );
  }
  for (const p of new SafeArrayIterator(permissions)) {
    if (typeof p !== "string") {
      throw new TypeError(
        "Cannot register persistent cron: 'permissions' entries must be strings",
      );
    }
  }

  const envObj = options.env;
  const env: [string, string][] = [];
  if (envObj !== undefined) {
    if (envObj === null || typeof envObj !== "object") {
      throw new TypeError(
        "Cannot register persistent cron: 'env' must be an object",
      );
    }
    for (const key of new SafeArrayIterator(ObjectKeys(envObj))) {
      if (!ObjectPrototypeHasOwnProperty(envObj, key)) continue;
      const value = envObj[key];
      if (typeof value !== "string") {
        throw new TypeError(
          `Cannot register persistent cron: env value for '${key}' must be a string`,
        );
      }
      ArrayPrototypePush(env, [key, value]);
    }
    // Sort for stable on-disk representation.
    ArrayPrototypeSort(
      env,
      (a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0),
    );
  }

  op_cron_persistent_create(
    name,
    scheduleStr,
    script,
    permissions,
    options.cwd,
    env,
  );
  return PromiseResolve();
}

function remove(name: string): Promise<void> {
  if (typeof name !== "string" || name.length === 0) {
    throw new TypeError(
      "Cannot remove persistent cron: 'name' must be a non-empty string",
    );
  }
  op_cron_persistent_remove(name);
  return PromiseResolve();
}

function list(): Promise<
  { name: string; schedule: string; script: string }[]
> {
  return PromiseResolve(op_cron_persistent_list());
}

// Attach persistent-cron methods as properties of the `cron` function so
// callers can do `Deno.cron.persistent(...)`.
(cron as unknown as {
  persistent: typeof persistent;
  remove: typeof remove;
  list: typeof list;
}).persistent = persistent;
(cron as unknown as { remove: typeof remove }).remove = remove;
(cron as unknown as { list: typeof list }).list = list;

// For testing
internals.formatToCronSchedule = formatToCronSchedule;
internals.parseScheduleToString = parseScheduleToString;

return { cron, formatToCronSchedule, parseScheduleToString };
})();
