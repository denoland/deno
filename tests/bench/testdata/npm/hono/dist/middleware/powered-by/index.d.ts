import type { Context } from '../../context';
import type { Next } from '../../hono';
export declare const poweredBy: () => (c: Context, next: Next) => Promise<void>;
