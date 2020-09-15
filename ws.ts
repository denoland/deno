import { delay } from "https://deno.land/std@0.65.0/async/delay.ts";

export interface ResolvableMethods<T> {
    resolve: (value?: T | PromiseLike<T>) => void;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    reject: (reason?: any) => void;
  }
  
  export type Resolvable<T> = Promise<T> & ResolvableMethods<T>;
  
  export function createResolvable<T>(): Resolvable<T> {
    let methods: ResolvableMethods<T>;
    const promise = new Promise<T>((resolve, reject): void => {
      methods = { resolve, reject };
    });
    // TypeScript doesn't know that the Promise callback occurs synchronously
    // therefore use of not null assertion (`!`)
    return Object.assign(promise, methods!) as Resolvable<T>;
  }

  const promise = createResolvable();

const ws = new WebSocket("wss://irc-ws.chat.twitch.tv:443");

ws.onopen = () => {
    console.log("connection opened");
    promise.resolve()
}

ws.onclose = () => {
    console.log("conn closed");
    console.table(Deno.resources());
    console.table(Deno.metrics());
    setTimeout(() => {
        console.table(Deno.resources());
        console.table(Deno.metrics());
    }, 2000);
}

console.log("before delay");
await promise;
console.log("after delay");

ws.close();
console.log("after close");