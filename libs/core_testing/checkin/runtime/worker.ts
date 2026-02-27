// Copyright 2018-2025 the Deno authors. MIT license.
import {
  op_worker_await_close,
  op_worker_parent,
  op_worker_recv,
  op_worker_send,
  op_worker_spawn,
  op_worker_terminate,
} from "ext:core/ops";

const privateConstructor = Symbol();
let parentWorker: Worker | null = null;

export class Worker {
  // deno-lint-ignore no-explicit-any
  #worker: any;

  // deno-lint-ignore no-explicit-any
  constructor(privateParent: symbol, worker: any);
  constructor(baseUrl: string, url: string);
  constructor(arg1: unknown, arg2: unknown) {
    if (arg1 == privateConstructor) {
      this.#worker = arg2;
    } else {
      this.#worker = op_worker_spawn(arg1, arg2);
    }
  }

  sendMessage(message: string) {
    op_worker_send(this.#worker, message);
  }

  async receiveMessage(): Promise<string | undefined> {
    return await op_worker_recv(this.#worker);
  }

  terminate() {
    op_worker_terminate(this.#worker);
  }

  get closed(): Promise<void> {
    return op_worker_await_close(this.#worker);
  }

  static get parent(): Worker {
    if (parentWorker === null) {
      parentWorker = new Worker(privateConstructor, op_worker_parent());
    }
    return parentWorker;
  }
}
