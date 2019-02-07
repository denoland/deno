export interface Storage {
  readonly length: number;

  key(index: number): string;
  getItem(key: string): string;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
  clear(): void;
}

export class SessionStorage implements Storage {
  private _storage: Record<string, string> = {};

  get length(): number {
    return Object.keys(this._storage).length;
  }

  key(index: number): string {
    return Object.keys(this._storage)[index];
  }
  getItem(key: string): string {
    return this._storage[key];
  }
  setItem(key: string, value: string): void {
    this._storage[key] = value;
  }
  removeItem(key: string): void {
    delete this._storage[key];
  }
  clear(): void {
    this._storage = {};
  }
}
