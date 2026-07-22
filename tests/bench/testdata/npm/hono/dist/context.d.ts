/// <reference types="@cloudflare/workers-types" />
import type { ContextVariableMap, NotFoundHandler } from './hono';
import type { CookieOptions } from './utils/cookie';
import type { StatusCode } from './utils/http-status';
declare type Headers = Record<string, string>;
export declare type Data = string | ArrayBuffer | ReadableStream;
declare type Env = Record<string, any>;
export interface Context<RequestParamKeyType extends string = string, E = Env> {
    req: Request<RequestParamKeyType>;
    env: E;
    event: FetchEvent;
    executionCtx: ExecutionContext;
    finalized: boolean;
    get res(): Response;
    set res(_res: Response);
    header: (name: string, value: string) => void;
    status: (status: StatusCode) => void;
    set: {
        <Key extends keyof ContextVariableMap>(key: Key, value: ContextVariableMap[Key]): void;
        (key: string, value: any): void;
    };
    get: {
        <Key extends keyof ContextVariableMap>(key: Key): ContextVariableMap[Key];
        <T = any>(key: string): T;
    };
    pretty: (prettyJSON: boolean, space?: number) => void;
    newResponse: (data: Data | null, status: StatusCode, headers: Headers) => Response;
    body: (data: Data | null, status?: StatusCode, headers?: Headers) => Response;
    text: (text: string, status?: StatusCode, headers?: Headers) => Response;
    json: <T>(object: T, status?: StatusCode, headers?: Headers) => Response;
    html: (html: string, status?: StatusCode, headers?: Headers) => Response;
    redirect: (location: string, status?: StatusCode) => Response;
    cookie: (name: string, value: string, options?: CookieOptions) => void;
    notFound: () => Response | Promise<Response>;
}
export declare class HonoContext<RequestParamKeyType extends string = string, E = Env> implements Context<RequestParamKeyType, E> {
    req: Request<RequestParamKeyType>;
    env: E;
    finalized: boolean;
    _status: StatusCode;
    private _executionCtx;
    private _pretty;
    private _prettySpace;
    private _map;
    private _headers;
    private _res;
    private notFoundHandler;
    constructor(req: Request, env?: E | undefined, executionCtx?: FetchEvent | ExecutionContext | undefined, notFoundHandler?: NotFoundHandler);
    get event(): FetchEvent;
    get executionCtx(): ExecutionContext;
    get res(): Response;
    set res(_res: Response);
    header(name: string, value: string): void;
    status(status: StatusCode): void;
    set<Key extends keyof ContextVariableMap>(key: Key, value: ContextVariableMap[Key]): void;
    set(key: string, value: any): void;
    get<Key extends keyof ContextVariableMap>(key: Key): ContextVariableMap[Key];
    get<T = any>(key: string): T;
    pretty(prettyJSON: boolean, space?: number): void;
    newResponse(data: Data | null, status: StatusCode, headers?: Headers): Response;
    body(data: Data | null, status?: StatusCode, headers?: Headers): Response;
    text(text: string, status?: StatusCode, headers?: Headers): Response;
    json<T>(object: T, status?: StatusCode, headers?: Headers): Response;
    html(html: string, status?: StatusCode, headers?: Headers): Response;
    redirect(location: string, status?: StatusCode): Response;
    cookie(name: string, value: string, opt?: CookieOptions): void;
    notFound(): Response | Promise<Response>;
}
export {};
