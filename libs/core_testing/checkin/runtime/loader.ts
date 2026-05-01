// Copyright 2018-2026 the Deno authors. MIT license.

const core = Deno.core;

interface RegisterOptions {
  resolve?: (
    specifier: string,
    context: { parentURL?: string },
    nextResolve: (
      specifier: string,
      context?: { parentURL?: string },
    ) => { url: string },
  ) => Promise<{ url: string }> | { url: string };
  load?: (
    url: string,
    context: Record<string, never>,
    nextLoad: (url: string) => { source: null },
  ) => Promise<{ source: string | null }> | { source: string | null };
}

/**
 * Register module loader hooks, similar to Node's `module.register()`.
 *
 * The `resolve` hook intercepts module resolution and can return a custom URL.
 * The `load` hook intercepts module loading and can return custom source code.
 * Both hooks receive a `next*` function to delegate to the default behavior.
 */
export function register(hooks: RegisterOptions): void {
  core.ops.op_loader_register(!!hooks.resolve, !!hooks.load);

  if (hooks.resolve) {
    const resolveHook = hooks.resolve;
    (async () => {
      while (true) {
        const pollPromise = core.ops.op_loader_poll_resolve();
        core.unrefOpPromise(pollPromise);
        const req = await pollPromise;
        if (req === null) break;
        const [id, specifier, referrer] = req;
        const context = { parentURL: referrer || undefined };
        const nextResolve = (
          spec: string,
          ctx?: { parentURL?: string },
        ) => {
          const parentURL = ctx?.parentURL ?? referrer;
          const url = core.ops.op_loader_default_resolve(spec, parentURL);
          return { url };
        };
        try {
          const result = await resolveHook(specifier, context, nextResolve);
          core.ops.op_loader_respond_resolve(id, result.url, null);
        } catch (e) {
          core.ops.op_loader_respond_resolve(id, null, String(e));
        }
      }
    })();
  }

  if (hooks.load) {
    const loadHook = hooks.load;
    (async () => {
      while (true) {
        const pollPromise = core.ops.op_loader_poll_load();
        core.unrefOpPromise(pollPromise);
        const req = await pollPromise;
        if (req === null) break;
        const [id, url] = req;
        const context = {};
        const nextLoad = (_url: string) => {
          // Signal to the Rust side to use default loading
          return { source: null as string | null };
        };
        try {
          const result = await loadHook(url, context, nextLoad);
          core.ops.op_loader_respond_load(id, result.source, null);
        } catch (e) {
          core.ops.op_loader_respond_load(id, null, String(e));
        }
      }
    })();
  }
}
