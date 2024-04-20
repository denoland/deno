function d() {
    let methods;
    const promise = new Promise((resolve, reject)=>{
        methods = {
            resolve,
            reject
        };
    });
    return Object.assign(promise, methods);
}
class A {
    s = d();
    a() {
        this.s.resolve();
    }
}
new A();
