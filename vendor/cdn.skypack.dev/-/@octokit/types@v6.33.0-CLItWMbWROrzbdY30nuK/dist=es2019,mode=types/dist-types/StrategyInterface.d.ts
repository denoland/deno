import { AuthInterface } from "./AuthInterface.d.ts";
export interface StrategyInterface<
  StrategyOptions extends any[],
  AuthOptions extends any[],
  Authentication extends object,
> {
  (...args: StrategyOptions): AuthInterface<AuthOptions, Authentication>;
}
