import type { ErrorHandler, NotFoundHandler } from './hono';
export declare const compose: <C>(middleware: Function[], onError?: ErrorHandler, onNotFound?: NotFoundHandler) => (context: C, next?: Function) => Promise<C>;
