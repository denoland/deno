/// <reference types="@cloudflare/workers-types" />
import type { Handler } from '../../hono';
export declare type ServeStaticOptions = {
    root?: string;
    path?: string;
    manifest?: object | string;
    namespace?: KVNamespace;
};
export declare const serveStatic: (options?: ServeStaticOptions) => Handler;
