const usb = Deno.core.dlopen(
  "node_modules/usb-enum/usb-enum.node",
);
console.log(await usb.list());