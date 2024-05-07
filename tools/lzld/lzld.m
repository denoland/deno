#import <dlfcn.h>

// -- QuartzCore.framework

extern void *kCAGravityTopLeft = 0;

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

