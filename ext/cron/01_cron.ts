// Copyright 2018-2026 the Deno authors. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_cron_compute_next_deadline,
  op_cron_get_net_handler,
} from "ext:core/ops";
const { internalRidSymbol } = core;
const {
  ArrayPrototypeJoin,
  NumberPrototypeToString,
  TypeError,
  Promise,
  DateNow,
  SafeMap,
  ArrayPrototypePush,
  ArrayPrototypeShift,
  SymbolDispose,
  ArrayPrototypeSome,
  SafeRegExp,
  PromiseWithResolvers,
  PromisePrototypeThen,
  SafePromiseRace,
  SafeSet,
  JSONStringify,
  JSONParse,
  SetPrototypeForEach,
} = primordials;
import {
  builtinTracer,
  enterSpan,
  restoreSnapshot,
  TRACING_ENABLED,
} from "ext:deno_telemetry/telemetry.ts";
import { updateSpanFromError } from "ext:deno_telemetry/util.ts";
import { clearTimeout, setTimeout } from "ext:deno_web/02_timers.js";
import { serveHttpOnListener } from "ext:deno_http/00_serve.ts";

const MAX_CRONS = 100;
const MAX_CRON_NAME = 64;
const DISPATCH_CONCURRENCY_LIMIT = 50;
const MAX_BACKOFF_MS = 60 * 60 * 1_000; // 1 hour
const MAX_BACKOFF_COUNT = 5;
const DEFAULT_BACKOFF_SCHEDULE = [100, 1_000, 5_000, 30_000, 60_000];
const NAME_REGEX = new SafeRegExp("^[a-zA-Z0-9-_ ]+$", "u");

export function formatToCronSchedule(
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

export function parseScheduleToString(
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

let CONCURRENCY = 0;
const CONCURRENCY_QUEUE: Array<() => void> = [];
async function acquireConcurrency() {
  if (CONCURRENCY >= DISPATCH_CONCURRENCY_LIMIT) {
    await new Promise<void>((r) => {
      ArrayPrototypePush(CONCURRENCY_QUEUE, r);
    });
  }
  CONCURRENCY += 1;
  return {
    [SymbolDispose]() {
      CONCURRENCY -= 1;
      ArrayPrototypeShift(CONCURRENCY_QUEUE)?.();
    },
  };
}

async function executeCron(
  name: string,
  schedule: string,
  handler: () => Promise<void> | void,
): Promise<boolean> {
  using _permit = await acquireConcurrency();

  let span;
  if (TRACING_ENABLED) {
    span = builtinTracer().startSpan("deno.cron", { kind: 0 });
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
    return true;
  } catch (error) {
    if (span) {
      updateSpanFromError(span, error);
      span.end();
    }
    import.meta.log("error", `Exception in cron handler ${name}`, error);
    return false;
  }
}

const CRONS: Map<
  string,
  {
    schedule: string;
    backoffSchedule: number[] | undefined;
    handler: () => Promise<void> | void;
  }
> = new SafeMap();

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

  if (name.length > MAX_CRON_NAME) {
    throw new TypeError(
      `Cron name cannot exceed ${MAX_CRON_NAME} characters: current length ${name.length}`,
    );
  }
  if (!NAME_REGEX.test(name)) {
    throw new TypeError(
      "Invalid cron name: only alphanumeric characters, whitespace, hyphens, and underscores are allowed",
    );
  }

  if (CRONS.size > MAX_CRONS) {
    throw new TypeError("Too many crons");
  }
  if (CRONS.has(name)) {
    throw new TypeError("Cron with this name already exists");
  }

  if (options?.backoffSchedule) {
    if (options.backoffSchedule.length > MAX_BACKOFF_COUNT) {
      throw new TypeError("Invalid backoff schedule");
    }
    if (
      ArrayPrototypeSome(options.backoffSchedule, (s) => s > MAX_BACKOFF_MS)
    ) {
      throw new TypeError("Invalid backoff schedule");
    }
  }

  // will throw on invalid schedule
  op_cron_compute_next_deadline(schedule);

  if (MAIN_READY_FLAG) {
    throw new TypeError("Deno.cron must be called at top-level.");
  }

  CRONS.set(name, {
    schedule,
    backoffSchedule: options?.backoffSchedule,
    handler,
  });

  const aborted = new Promise<void>((resolve) => {
    options?.signal?.addEventListener("abort", () => {
      CRONS.delete(name);
      resolve();
    }, { once: true });
  });

  if (setupSock()) {
    return aborted;
  }

  return (async () => {
    let success = true;
    let currentExecutionRetries = 0;
    while (true) {
      if (options?.signal?.aborted) {
        break;
      }

      const backoffSchedule = options?.backoffSchedule ??
        DEFAULT_BACKOFF_SCHEDULE;
      let delta;
      if (!success && currentExecutionRetries < backoffSchedule.length) {
        delta = backoffSchedule[currentExecutionRetries];
        currentExecutionRetries += 1;
      } else {
        delta = op_cron_compute_next_deadline(schedule) - DateNow();
        currentExecutionRetries = 0;
      }

      if (delta > 0) {
        let timeout: number;
        const alive = await SafePromiseRace([
          new Promise((resolve) => {
            timeout = setTimeout(() => resolve(true), delta);
          }),
          PromisePrototypeThen(aborted, () => {
            clearTimeout(timeout);
            return false;
          }),
        ]);

        if (!alive) {
          break;
        }
      }

      success = await executeCron(name, schedule, handler);
    }
  })();
}

let RAN_SETUP = false;
let USING_SOCK = false;
function setupSock(): boolean {
  if (RAN_SETUP) return USING_SOCK;
  RAN_SETUP = true;
  const rid = op_cron_get_net_handler();
  if (rid) {
    USING_SOCK = true;
    runOnSock(rid);
  }
  return USING_SOCK;
}

const MAIN_READY = PromiseWithResolvers();
let MAIN_READY_FLAG = false;
export function setMainReady() {
  MAIN_READY_FLAG = true;
  MAIN_READY.resolve();
}

function runOnSock(rid: number) {
  const connections = new SafeSet();

  const onRequest = async (req: Request) => {
    await MAIN_READY.promise;

    const { socket, response } = Deno.upgradeWebSocket(req);

    connections.add(socket);
    socket.addEventListener("close", () => connections.remove(socket), {
      once: true,
    });

    socket.addEventListener("open", () => {
      const crons: Record<
        string,
        { schedule: string; backoffSchedule: number[] | undefined }
      > = {};
      // CRONS is a SafeMap so we can iterate it
      // deno-lint-ignore prefer-primordials
      for (const { 0: k, 1: v } of CRONS) {
        crons[k] = {
          schedule: v.schedule,
          backoffSchedule: v.backoffSchedule,
        };
      }
      socket.send(JSONStringify({ type: "crons", crons }));
    }, { once: true });

    socket.addEventListener("message", (event) => {
      const { type, id, ...args } = JSONParse(event.data);
      switch (type) {
        case "execute": {
          const cron = CRONS.get(args.name);
          if (cron) {
            PromisePrototypeThen(
              executeCron(args.name, cron.schedule, cron.handler),
              (success) => {
                SetPrototypeForEach(connections, (s) => {
                  s.send(JSONStringify({ type: "result", id, success }));
                });
              },
            );
          } else {
            socket.send(JSONStringify({ type: "result", id, success: false }));
          }
          break;
        }
        default:
          break;
      }
    });

    return response;
  };

  return serveHttpOnListener(
    { [internalRidSymbol]: rid },
    null,
    onRequest,
    (error) => {
      import.meta.log("error", error);
      new Response("Internal server error", { status: 500 });
    },
    () => {},
  );
}

// For testing
internals.formatToCronSchedule = formatToCronSchedule;
internals.parseScheduleToString = parseScheduleToString;
internals.allowCronRegistrationAfterStartup = () => {
  MAIN_READY_FLAG = false;
};

export { cron };
