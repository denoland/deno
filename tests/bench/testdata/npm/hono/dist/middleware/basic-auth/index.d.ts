import type { Context } from '../../context';
import type { Next } from '../../hono';
export declare const basicAuth: (options: {
    username: string;
    password: string;
    realm?: string;
    hashFunction?: Function;
}, ...users: {
    username: string;
    password: string;
}[]) => (ctx: Context, next: Next) => Promise<void>;
