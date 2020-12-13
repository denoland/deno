const data = 0;

const main = async (): Promise<void> => {
  let resolve: (value: number) => void;

  const workerUrl = new URL('./worker_post_undefined_worker.ts', import.meta.url).href;
  const worker = new Worker(workerUrl, {deno: true, type: 'module'});

  const handleWorkerMessage = (ev: MessageEvent): void => {
    const {data} = ev;
    console.log('main <- worker:', data);
    resolve(data);
    worker.terminate();
  };

  worker.addEventListener('messageerror', () => console.log('message error'));
  worker.addEventListener('error', () => console.log('error'));
  worker.addEventListener('message', handleWorkerMessage);

  const result = await new Promise(res => {
    resolve = res;
    worker.postMessage(data);
  });

  console.log(result);
};

if (import.meta.main) main();
