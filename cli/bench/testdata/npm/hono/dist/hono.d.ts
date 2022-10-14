/// <reference types="@cloudflare/workers-types" />
import type { Context } from './context';
import type { Router } from './router';
export interface ContextVariableMap {
}
declare type Env = Record<string, any>;
export declare type Handler<RequestParamKeyType extends string = string, E = Env> = (c: Context<RequestParamKeyType, E>, next: Next) => Response | Promise<Response> | Promise<void> | Promise<Response | undefined>;
export declare type NotFoundHandler<E = Env> = (c: Context<string, E>) => Response | Promise<Response>;
export declare type ErrorHandler<E = Env> = (err: Error, c: Context<string, E>) => Response;
export declare type Next = () => Promise<void>;
declare type ParamKeyName<NameWithPattern> = NameWithPattern extends `${infer Name}{${infer _Pattern}` ? Name : NameWithPattern;
declare type ParamKey<Component> = Component extends `:${infer NameWithPattern}` ? ParamKeyName<NameWithPattern> : never;
declare type ParamKeys<Path> = Path extends `${infer Component}/${infer Rest}` ? ParamKey<Component> | ParamKeys<Rest> : ParamKey<Path>;
interface HandlerInterface<T extends string, E extends Env = Env, U = Hono<E, T>> {
    <Path extends string>(path: Path, ...handlers: Handler<ParamKeys<Path> extends never ? string : ParamKeys<Path>, E>[]): U;
    (path: string, ...handlers: Handler<string, E>[]): U;
    <Path extends string>(...handlers: Handler<ParamKeys<Path> extends never ? string : ParamKeys<Path>, E>[]): U;
    (...handlers: Handler<string, E>[]): U;
}
interface Route<E extends Env> {
    path: string;
    method: string;
    handler: Handler<string, E>;
}
declare const Hono_base: new <E_1 extends Env, T extends string, U>() => {
    all: HandlerInterface<T, E_1, U>;
    get: HandlerInterface<T, E_1, U>;
    post: HandlerInterface<T, E_1, U>;
    put: HandlerInterface<T, E_1, U>;
    delete: HandlerInterface<T, E_1, U>;
    head: HandlerInterface<T, E_1, U>;
    options: HandlerInterface<T, E_1, U>;
    patch: HandlerInterface<T, E_1, U>;
};
export declare class Hono<E extends Env = Env, P extends string = '/'> extends Hono_base<E, P, Hono<E, P>> {
    readonly router: Router<Handler<string, E>>;
    readonly strict: boolean;
    private _tempPath;
    private path;
    routes: Route<E>[];
    constructor(init?: Partial<Pick<Hono, 'router' | 'strict'>>);
    private notFoundHandler;
    private errorHandler;
    route(path: string, app?: Hono<any>): Hono<E, P>;
    use(path: string, ...middleware: Handler<string, E>[]): Hono<E, P>;
    use(...middleware: Handler<string, E>[]): Hono<E, P>;
    onError(handler: ErrorHandler<E>): Hono<E, P>;
    notFound(handler: NotFoundHandler<E>): Hono<E, P>;
    private addRoute;
    private matchRoute;
    private dispatch;
    handleEvent(event: FetchEvent): Promise<Response>;
    fetch: (request: Request, env?: E, executionCtx?: ExecutionContext) => Promise<Response>;
    request(input: RequestInfo, requestInit?: RequestInit): Promise<Response>;
}
export {};
