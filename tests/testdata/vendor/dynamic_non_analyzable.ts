const value = (() => "./logger.ts")();
const { Logger } = await import(value);

export { Logger };
