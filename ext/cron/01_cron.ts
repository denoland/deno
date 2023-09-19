// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-ignore internal api
const core = Deno.core;

function cron(
  name: string,
  schedule: string,
  handler: () => Promise<void> | void,
  options?: { backoffSchedule?: number[]; signal?: AbortSignal },
) {
  if (name === undefined) {
    throw new TypeError("Deno.cron requires a unique name");
  }
  if (schedule === undefined) {
    throw new TypeError("Deno.cron requires a valid schedule");
  }

  const rid = core.ops.op_cron_create(
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
      const r = await core.opAsync("op_cron_next", rid, success);
      if (r === false) {
        break;
      }
      try {
        const result = handler();
        const _res = result instanceof Promise ? (await result) : result;
        success = true;
      } catch (error) {
        console.error(`Exception in cron handler ${name}`, error);
        success = false;
      }
    }
  })();
}

export { cron };
