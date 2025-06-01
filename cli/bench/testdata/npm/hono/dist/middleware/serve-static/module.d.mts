import type { ServeStaticOptions } from './serve-static';
declare const module: (options?: ServeStaticOptions) => import("../../hono").Handler<string, {
    [x: string]: any;
}>;
export { module as serveStatic };
