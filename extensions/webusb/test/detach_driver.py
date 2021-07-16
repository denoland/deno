import usb.core

dev = usb.core.find() # TODO: filter
dev.detach_kernel_driver(0);