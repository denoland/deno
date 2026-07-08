export interface Config {
  timeout: number;
  retries: number;
}

export type Status = "ok" | "error";
