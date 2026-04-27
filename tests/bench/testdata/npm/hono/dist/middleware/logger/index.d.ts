import type { Context } from '../../context';
import type { Next } from '../../hono';
declare type PrintFunc = (str: string, ...rest: string[]) => void;
export declare const logger: (fn?: PrintFunc) => (c: Context, next: Next) => Promise<void>;
export {};
