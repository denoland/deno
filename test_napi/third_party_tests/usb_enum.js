const usb = Deno.core.napiOpen(
  "node_modules/usb-enum/usb-enum.node",
);
console.log(await usb.list());
