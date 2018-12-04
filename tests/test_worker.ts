interface Worker {
    onmessage: ((this: Worker, ev: any) => any) | null;
}

declare var Worker: {
    prototype: Worker;
    new(stringUrl: string, options?: any): Worker;
};

const worker = new Worker('./worker.ts');

worker.onmessage = args => {
    console.log('worker onmessage', args);
};