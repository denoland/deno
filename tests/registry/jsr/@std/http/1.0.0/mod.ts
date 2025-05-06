// Copyright 2018-2025 the Deno authors. MIT license.

/**
 * Request handler for {@linkcode Route}.
 *
 * > [!WARNING]
 * > **UNSTABLE**: New API, yet to be vetted.
 *
 * @experimental
 *
 * Extends {@linkcode Deno.ServeHandlerInfo} by adding adding a `params` argument.
 *
 * @param request Request
 * @param info Request info
 * @param params URL pattern result
 */
export type Handler = (
  request: Request,
  info?: Deno.ServeHandlerInfo,
  params?: URLPatternResult | null,
) => Response | Promise<Response>;

/**
 * Route configuration for {@linkcode route}.
 *
 * > [!WARNING]
 * > **UNSTABLE**: New API, yet to be vetted.
 *
 * @experimental
 */
export interface Route {
  /**
   * Request URL pattern.
   */
  pattern: URLPattern;
  /**
   * Request method.
   *
   * @default {"GET"}
   */
  method?: string;
  /**
   * Request handler.
   */
  handler: Handler;
}

/**
 * Routes requests to different handlers based on the request path and method.
 *
 * > [!WARNING]
 * > **UNSTABLE**: New API, yet to be vetted.
 *
 * @experimental
 *
 * @example Usage
 * ```ts no-eval
 * import { route, type Route } from "@std/http/route";
 * import { serveDir } from "@std/http/file-server";
 *
 * const routes: Route[] = [
 *   {
 *     pattern: new URLPattern({ pathname: "/about" }),
 *     handler: () => new Response("About page"),
 *   },
 *   {
 *     pattern: new URLPattern({ pathname: "/users/:id" }),
 *     handler: (_req, _info, params) => new Response(params?.pathname.groups.id),
 *   },
 *   {
 *     pattern: new URLPattern({ pathname: "/static/*" }),
 *     handler: (req: Request) => serveDir(req)
 *   }
 * ];
 *
 * function defaultHandler(_req: Request) {
 *   return new Response("Not found", { status: 404 });
 * }
 *
 * Deno.serve(route(routes, defaultHandler));
 * ```
 *
 * @param routes Route configurations
 * @param defaultHandler Default request handler that's returned when no route
 * matches the given request. Serving HTTP 404 Not Found or 405 Method Not
 * Allowed response can be done in this function.
 * @returns Request handler
 */
export function route(
  routes: Route[],
  defaultHandler: (
    request: Request,
    info?: Deno.ServeHandlerInfo,
  ) => Response | Promise<Response>,
): (
  request: Request,
  info?: Deno.ServeHandlerInfo,
) => Response | Promise<Response> {
  // TODO(iuioiua): Use `URLPatternList` once available (https://github.com/whatwg/urlpattern/pull/166)
  return (request: Request, info?: Deno.ServeHandlerInfo) => {
    for (const route of routes) {
      const match = route.pattern.exec(request.url);
      if (match) return route.handler(request, info, match);
    }
    return defaultHandler(request, info);
  };
}

interface ServeDirOptions {
  urlRoot?: string;
}

// NOTE(bartlomieju): not important, just for testing
export async function serveDir(req: Request, opts: ServeDirOptions = {}): Response | Promise<Response> {
  return new Response("hello world")
}
