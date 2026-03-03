import type { Result } from '../../router';
import type { Pattern } from '../../utils/url';
declare type HandlerSet<T> = {
    handler: T;
    score: number;
    name: string;
};
export declare class Node<T> {
    methods: Record<string, HandlerSet<T>>[];
    children: Record<string, Node<T>>;
    patterns: Pattern[];
    order: number;
    name: string;
    handlerSetCache: Record<string, HandlerSet<T>[]>;
    constructor(method?: string, handler?: T, children?: Record<string, Node<T>>);
    insert(method: string, path: string, handler: T): Node<T>;
    private getHandlerSets;
    search(method: string, path: string): Result<T> | null;
}
export {};
