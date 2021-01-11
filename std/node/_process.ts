// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { notImplemented } from "./_utils.ts";
import EventEmitter from "./events.ts";
import { fromFileUrl } from "../path/mod.ts";

const notImplementedEvents = [
  "beforeExit",
  "disconnect",
  "message",
  "multipleResolves",
  "rejectionHandled",
  "SIGBREAK",
  "SIGBUS",
  "SIGFPE",
  "SIGHUP",
  "SIGILL",
  "SIGINT",
  "SIGSEGV",
  "SIGTERM",
  "SIGWINCH",
  "uncaughtException",
  "uncaughtExceptionMonitor",
  "unhandledRejection",
  "warning",
];

class Process extends EventEmitter {
  constructor() {
    super();

    //This causes the exit event to be binded to the unload event
    window.addEventListener("unload", () => {
      //TODO(Soremwar)
      //Get the exit code from the unload event
      super.emit("exit", 0);
    });
  }

  /** https://nodejs.org/api/process.html#process_process_arch */
  arch = Deno.build.arch;

  get data(){
    return [Deno.execPath(), fromFileUrl(Deno.mainModule), ...Deno.args];
  }

  //TODO(Soremwar)
  //Totally not the best way to do this
  #argv = (() => {
    //deno-lint-ignore ban-ts-comment
    //@ts-ignore
    const args: {
      [Deno.customInspect] : () => string,
      [key: number]: string,
    } = [];

    args[Deno.customInspect] = () => {
      return Deno.inspect(this.data, {
        colors: true,
      });
    };

    return args;
  })();

  /** https://nodejs.org/api/process.html#process_process_argv */
  argv: {[key: number]: string} = new Proxy(this.#argv, {
    get(target, prop){
      if(prop === Deno.customInspect){
        return target[Deno.customInspect];
      }
      //TODO(Soremwar)
      //This could be greatly improved if TS added support for private accessors
      return [Deno.execPath(), fromFileUrl(Deno.mainModule), ...Deno.args][prop as number];
    },
    ownKeys(){
      return Reflect.ownKeys([Deno.execPath(), fromFileUrl(Deno.mainModule), ...Deno.args]);
    },
  });

  /** https://nodejs.org/api/process.html#process_process_chdir_directory */
  chdir = Deno.chdir;

  /** https://nodejs.org/api/process.html#process_process_cwd */
  cwd = Deno.cwd;

  /** https://nodejs.org/api/process.html#process_process_exit_code */
  exit = Deno.exit;

  #env = {
    [Deno.customInspect]: function(){
      return Deno.inspect(Deno.env.toObject(), {
        colors: true,
      });
    },
  };

  /** https://nodejs.org/api/process.html#process_process_env */
  env: { [index: string]: string } = new Proxy(this.#env, {
    get(target, prop){
      if(prop === Deno.customInspect){
        return target[Deno.customInspect];
      }
      return Deno.env.get(String(prop));
    },
    ownKeys(){
      return Reflect.ownKeys(Deno.env.toObject());
    },
    set(_target, prop, value){
      Deno.env.set(String(prop), String(value));
      return value;
    },
  });

  /** https://nodejs.org/api/process.html#process_process_nexttick_callback_args */
  nextTick(this: unknown, cb: () => void): void;
  nextTick<T extends Array<unknown>>(
    this: unknown,
    cb: (...args: T) => void,
    ...args: T
  ): void;
  nextTick<T extends Array<unknown>>(
    this: unknown,
    cb: (...args: T) => void,
    ...args: T
  ) {
    if (args) {
      queueMicrotask(() => cb.call(this, ...args));
    } else {
      queueMicrotask(cb);
    }
  }

  /** https://nodejs.org/api/process.html#process_process_events */
  //deno-lint-ignore ban-types
  on(event: typeof notImplementedEvents[number], listener: Function): never;
  on(event: "exit", listener: (code: number) => void): this;
  //deno-lint-ignore no-explicit-any
  on(event: string, listener: (...args: any[]) => void): this {
    if (notImplementedEvents.includes(event)) {
      notImplemented();
    }

    super.on(event, listener);

    return this;
  }

  /** https://nodejs.org/api/process.html#process_process_pid */
  pid = Deno.pid;

  /** https://nodejs.org/api/process.html#process_process_platform */
  platform = Deno.build.os === "windows" ? "win32" : Deno.build.os;

  removeAllListeners(_event: string): never {
    notImplemented();
  }

  removeListener(
    event: typeof notImplementedEvents[number],
    //deno-lint-ignore ban-types
    listener: Function,
  ): never;
  removeListener(event: "exit", listener: (code: number) => void): this;
  //deno-lint-ignore no-explicit-any
  removeListener(event: string, listener: (...args: any[]) => void): this {
    if (notImplementedEvents.includes(event)) {
      notImplemented();
    }

    super.removeListener("exit", listener);

    return this;
  }

  /** https://nodejs.org/api/process.html#process_process_stderr */
  get stderr() {
    return {
      fd: Deno.stderr.rid,
      get isTTY(): boolean {
        return Deno.isatty(this.fd);
      },
      pipe(_destination: Deno.Writer, _options: { end: boolean }): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      write(_chunk: string | Uint8Array, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      on(_event: string, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
    };
  }

  /** https://nodejs.org/api/process.html#process_process_stdin */
  get stdin() {
    return {
      fd: Deno.stdin.rid,
      get isTTY(): boolean {
        return Deno.isatty(this.fd);
      },
      read(_size: number): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      on(_event: string, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
    };
  }

  /** https://nodejs.org/api/process.html#process_process_stdout */
  get stdout() {
    return {
      fd: Deno.stdout.rid,
      get isTTY(): boolean {
        return Deno.isatty(this.fd);
      },
      pipe(_destination: Deno.Writer, _options: { end: boolean }): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      write(_chunk: string | Uint8Array, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
      // deno-lint-ignore ban-types
      on(_event: string, _callback: Function): void {
        // TODO(JayHelton): to be implemented
        notImplemented();
      },
    };
  }

  /** https://nodejs.org/api/process.html#process_process_version */
  version = `v${Deno.version.deno}`;

  /** https://nodejs.org/api/process.html#process_process_versions */
  versions = {
    node: Deno.version.deno,
    ...Deno.version,
  };
}

/** https://nodejs.org/api/process.html#process_process */
const process = new Process();

Object.defineProperty(process, Symbol.toStringTag, {
  enumerable: false,
  writable: true,
  configurable: false,
  value: "process",
});

export default process;
