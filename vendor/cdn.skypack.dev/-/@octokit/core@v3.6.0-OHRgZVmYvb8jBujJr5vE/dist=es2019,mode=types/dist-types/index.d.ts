import { HookCollection } from "/-/before-after-hook@v2.2.2-pi5OVaqfPuA5i8u2q0Od/dist=es2019,mode=types/index.d.ts";
import { request } from "/-/@octokit/request@v5.6.3-8MiyZSoy8B73C1K9nYC8/dist=es2019,mode=types/index.d.ts";
import { graphql } from "/-/@octokit/graphql@v4.8.0-EbMgAhtBEVS5THey9fbY/dist=es2019,mode=types/index.d.ts";
import {
  Constructor,
  Hooks,
  OctokitOptions,
  OctokitPlugin,
  ReturnTypeOf,
  UnionToIntersection,
} from "./types.d.ts";
export declare class Octokit {
  static VERSION: string;
  static defaults<S extends Constructor<any>>(
    this: S,
    defaults: OctokitOptions | Function,
  ): S;
  static plugins: OctokitPlugin[];
  /**
   * Attach a plugin (or many) to your Octokit instance.
   *
   * @example
   * const API = Octokit.plugin(plugin1, plugin2, plugin3, ...)
   */
  static plugin<
    S extends Constructor<any> & {
      plugins: any[];
    },
    T extends OctokitPlugin[],
  >(
    this: S,
    ...newPlugins: T
  ): S & Constructor<UnionToIntersection<ReturnTypeOf<T>>>;
  constructor(options?: OctokitOptions);
  request: typeof request;
  graphql: typeof graphql;
  log: {
    debug: (message: string, additionalInfo?: object) => any;
    info: (message: string, additionalInfo?: object) => any;
    warn: (message: string, additionalInfo?: object) => any;
    error: (message: string, additionalInfo?: object) => any;
    [key: string]: any;
  };
  hook: HookCollection<Hooks>;
  auth: (...args: unknown[]) => Promise<unknown>;
}
