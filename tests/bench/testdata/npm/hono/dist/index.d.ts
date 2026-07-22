/// <reference path="request.d.ts" />
import { Hono } from './hono';
export type { Handler, Next } from './hono';
export type { Context } from './context';
declare module './hono' {
    interface Hono {
        fire(): void;
    }
}
export { Hono };
