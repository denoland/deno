import type { Result, Router } from '../../router';
import { Node } from './node';
export declare class TrieRouter<T> implements Router<T> {
    node: Node<T>;
    constructor();
    add(method: string, path: string, handler: T): void;
    match(method: string, path: string): Result<T> | null;
}
