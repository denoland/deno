const usb = Deno.core.dlopen(
  "./node_modules/usb/build/Release/usb_bindings.node",
);
await usb.getDeviceList();
