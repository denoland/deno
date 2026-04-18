import type { Context } from '../../context';
import type { Next } from '../../hono';
declare type prettyOptions = {
    space: number;
};
export declare const prettyJSON: (options?: prettyOptions) => (c: Context, next: Next) => Promise<void>;
export {};
