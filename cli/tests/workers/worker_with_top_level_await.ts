function delay(ms: number): Promise<void> {
  return new Promise<void>((resolve) => {
    setTimeout(() => {
      resolve();
    }, ms);
  });
}

onmessage = (e: MessageEvent) => {
  postMessage("triggered worker handler");
  close();
};
postMessage("ready");
await delay(1000);
postMessage("never");
