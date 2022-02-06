const usb = Deno.core.dlopen(
  "./node_modules/usb/prebuilds/linux-x64/node.napi.glibc.node",
);

console.log(usb.getDeviceList());
