/*
    lzld - lazy loading of OSX frmeworks

    This is compiled as a static library and linked against the binary via `lzld` script.
    This code dynamically load frameworks when symbol is called using dlopen and dlsym.

    Dependencies:
    - dlfcn.h: Header file providing functions for dynamic linking.
    - QuartzCore.framework: Provides graphics rendering support. (WebGPU)
    - Metal.framework: Framework for high-performance GPU-accelerated graphics and computing. (WebGPU)
*/

#import <dlfcn.h>

// -- QuartzCore.framework

void *kCAGravityTopLeft = 0;

// -- Metal.framework

void *(*MTLCopyAllDevices_)(void) = 0;

void loadMetalFramework() {
    void *handle = dlopen("/System/Library/Frameworks/Metal.framework/Metal", RTLD_LAZY);
    if (handle) {
        MTLCopyAllDevices_ = dlsym(handle, "MTLCopyAllDevices");
    }
}

extern void *MTLCopyAllDevices(void) {
    if (MTLCopyAllDevices_ == 0) {
        loadMetalFramework();
    }

    return MTLCopyAllDevices_();
}

