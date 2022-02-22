const usb = Deno.core.napiOpen(
  "./node_modules/usb/build/Release/usb_bindings.node",
);
await usb.getDeviceList();
