# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import collections
import contextlib
import logging
import os
import re
import shutil
import tempfile
import unittest

from pylib.symbols import apk_native_libs_unittest
from pylib.symbols import mock_addr2line
from pylib.symbols import symbol_utils

_MOCK_ELF_DATA = apk_native_libs_unittest.MOCK_ELF_DATA

_MOCK_A2L_PATH = os.path.join(os.path.dirname(mock_addr2line.__file__),
                              'mock_addr2line')


# pylint: disable=line-too-long

# list of (start_offset, end_offset, size, libpath) tuples corresponding
# to the content of base.apk. This was taken from an x86 ChromeModern.apk
# component build.
_TEST_APK_LIBS = [
  (0x01331000, 0x013696bc, 0x000386bc, 'libaccessibility.cr.so'),
  (0x0136a000, 0x013779c4, 0x0000d9c4, 'libanimation.cr.so'),
  (0x01378000, 0x0137f7e8, 0x000077e8, 'libapdu.cr.so'),
  (0x01380000, 0x0155ccc8, 0x001dccc8, 'libbase.cr.so'),
  (0x0155d000, 0x015ab98c, 0x0004e98c, 'libbase_i18n.cr.so'),
  (0x015ac000, 0x015dff4c, 0x00033f4c, 'libbindings.cr.so'),
  (0x015e0000, 0x015f5a54, 0x00015a54, 'libbindings_base.cr.so'),
  (0x015f6000, 0x0160d770, 0x00017770, 'libblink_android_mojo_bindings_shared.cr.so'),
  (0x0160e000, 0x01731960, 0x00123960, 'libblink_common.cr.so'),
  (0x01732000, 0x0174ce54, 0x0001ae54, 'libblink_controller.cr.so'),
  (0x0174d000, 0x0318c528, 0x01a3f528, 'libblink_core.cr.so'),
  (0x0318d000, 0x03191700, 0x00004700, 'libblink_core_mojo_bindings_shared.cr.so'),
  (0x03192000, 0x03cd7918, 0x00b45918, 'libblink_modules.cr.so'),
  (0x03cd8000, 0x03d137d0, 0x0003b7d0, 'libblink_mojo_bindings_shared.cr.so'),
  (0x03d14000, 0x03d2670c, 0x0001270c, 'libblink_offscreen_canvas_mojo_bindings_shared.cr.so'),
  (0x03d27000, 0x046c7054, 0x009a0054, 'libblink_platform.cr.so'),
  (0x046c8000, 0x0473fbfc, 0x00077bfc, 'libbluetooth.cr.so'),
  (0x04740000, 0x04878f40, 0x00138f40, 'libboringssl.cr.so'),
  (0x04879000, 0x0498466c, 0x0010b66c, 'libc++_shared.so'),
  (0x04985000, 0x0498d93c, 0x0000893c, 'libcaptive_portal.cr.so'),
  (0x0498e000, 0x049947cc, 0x000067cc, 'libcapture_base.cr.so'),
  (0x04995000, 0x04b39f18, 0x001a4f18, 'libcapture_lib.cr.so'),
  (0x04b3a000, 0x04b488ec, 0x0000e8ec, 'libcbor.cr.so'),
  (0x04b49000, 0x04e9ea5c, 0x00355a5c, 'libcc.cr.so'),
  (0x04e9f000, 0x04ed6404, 0x00037404, 'libcc_animation.cr.so'),
  (0x04ed7000, 0x04ef5ab4, 0x0001eab4, 'libcc_base.cr.so'),
  (0x04ef6000, 0x04fd9364, 0x000e3364, 'libcc_blink.cr.so'),
  (0x04fda000, 0x04fe2758, 0x00008758, 'libcc_debug.cr.so'),
  (0x04fe3000, 0x0500ae0c, 0x00027e0c, 'libcc_ipc.cr.so'),
  (0x0500b000, 0x05078f38, 0x0006df38, 'libcc_paint.cr.so'),
  (0x05079000, 0x0507e734, 0x00005734, 'libcdm_manager.cr.so'),
  (0x0507f000, 0x06f4d744, 0x01ece744, 'libchrome.cr.so'),
  (0x06f54000, 0x06feb830, 0x00097830, 'libchromium_sqlite3.cr.so'),
  (0x06fec000, 0x0706f554, 0x00083554, 'libclient.cr.so'),
  (0x07070000, 0x0708da60, 0x0001da60, 'libcloud_policy_proto_generated_compile.cr.so'),
  (0x0708e000, 0x07121f28, 0x00093f28, 'libcodec.cr.so'),
  (0x07122000, 0x07134ab8, 0x00012ab8, 'libcolor_space.cr.so'),
  (0x07135000, 0x07138614, 0x00003614, 'libcommon.cr.so'),
  (0x07139000, 0x0717c938, 0x00043938, 'libcompositor.cr.so'),
  (0x0717d000, 0x0923d78c, 0x020c078c, 'libcontent.cr.so'),
  (0x0923e000, 0x092ae87c, 0x0007087c, 'libcontent_common_mojo_bindings_shared.cr.so'),
  (0x092af000, 0x092be718, 0x0000f718, 'libcontent_public_common_mojo_bindings_shared.cr.so'),
  (0x092bf000, 0x092d9a20, 0x0001aa20, 'libcrash_key.cr.so'),
  (0x092da000, 0x092eda58, 0x00013a58, 'libcrcrypto.cr.so'),
  (0x092ee000, 0x092f16e0, 0x000036e0, 'libdevice_base.cr.so'),
  (0x092f2000, 0x092fe8d8, 0x0000c8d8, 'libdevice_event_log.cr.so'),
  (0x092ff000, 0x093026a4, 0x000036a4, 'libdevice_features.cr.so'),
  (0x09303000, 0x093f1220, 0x000ee220, 'libdevice_gamepad.cr.so'),
  (0x093f2000, 0x09437f54, 0x00045f54, 'libdevice_vr_mojo_bindings.cr.so'),
  (0x09438000, 0x0954c168, 0x00114168, 'libdevice_vr_mojo_bindings_blink.cr.so'),
  (0x0954d000, 0x0955d720, 0x00010720, 'libdevice_vr_mojo_bindings_shared.cr.so'),
  (0x0955e000, 0x0956b9c0, 0x0000d9c0, 'libdevices.cr.so'),
  (0x0956c000, 0x0957cae8, 0x00010ae8, 'libdiscardable_memory_client.cr.so'),
  (0x0957d000, 0x09588854, 0x0000b854, 'libdiscardable_memory_common.cr.so'),
  (0x09589000, 0x0959cbb4, 0x00013bb4, 'libdiscardable_memory_service.cr.so'),
  (0x0959d000, 0x095b6b90, 0x00019b90, 'libdisplay.cr.so'),
  (0x095b7000, 0x095be930, 0x00007930, 'libdisplay_types.cr.so'),
  (0x095bf000, 0x095c46c4, 0x000056c4, 'libdisplay_util.cr.so'),
  (0x095c5000, 0x095f54a4, 0x000304a4, 'libdomain_reliability.cr.so'),
  (0x095f6000, 0x0966fe08, 0x00079e08, 'libembedder.cr.so'),
  (0x09670000, 0x096735f8, 0x000035f8, 'libembedder_switches.cr.so'),
  (0x09674000, 0x096a3460, 0x0002f460, 'libevents.cr.so'),
  (0x096a4000, 0x096b6d40, 0x00012d40, 'libevents_base.cr.so'),
  (0x096b7000, 0x0981a778, 0x00163778, 'libffmpeg.cr.so'),
  (0x0981b000, 0x09945c94, 0x0012ac94, 'libfido.cr.so'),
  (0x09946000, 0x09a330dc, 0x000ed0dc, 'libfingerprint.cr.so'),
  (0x09a34000, 0x09b53170, 0x0011f170, 'libfreetype_harfbuzz.cr.so'),
  (0x09b54000, 0x09bc5c5c, 0x00071c5c, 'libgcm.cr.so'),
  (0x09bc6000, 0x09cc8584, 0x00102584, 'libgeolocation.cr.so'),
  (0x09cc9000, 0x09cdc8d4, 0x000138d4, 'libgeometry.cr.so'),
  (0x09cdd000, 0x09cec8b4, 0x0000f8b4, 'libgeometry_skia.cr.so'),
  (0x09ced000, 0x09d10e14, 0x00023e14, 'libgesture_detection.cr.so'),
  (0x09d11000, 0x09d7595c, 0x0006495c, 'libgfx.cr.so'),
  (0x09d76000, 0x09d7d7cc, 0x000077cc, 'libgfx_ipc.cr.so'),
  (0x09d7e000, 0x09d82708, 0x00004708, 'libgfx_ipc_buffer_types.cr.so'),
  (0x09d83000, 0x09d89748, 0x00006748, 'libgfx_ipc_color.cr.so'),
  (0x09d8a000, 0x09d8f6f4, 0x000056f4, 'libgfx_ipc_geometry.cr.so'),
  (0x09d90000, 0x09d94754, 0x00004754, 'libgfx_ipc_skia.cr.so'),
  (0x09d95000, 0x09d9869c, 0x0000369c, 'libgfx_switches.cr.so'),
  (0x09d99000, 0x09dba0ac, 0x000210ac, 'libgin.cr.so'),
  (0x09dbb000, 0x09e0a8cc, 0x0004f8cc, 'libgl_in_process_context.cr.so'),
  (0x09e0b000, 0x09e17a18, 0x0000ca18, 'libgl_init.cr.so'),
  (0x09e18000, 0x09ee34e4, 0x000cb4e4, 'libgl_wrapper.cr.so'),
  (0x09ee4000, 0x0a1a2e00, 0x002bee00, 'libgles2.cr.so'),
  (0x0a1a3000, 0x0a24556c, 0x000a256c, 'libgles2_implementation.cr.so'),
  (0x0a246000, 0x0a267038, 0x00021038, 'libgles2_utils.cr.so'),
  (0x0a268000, 0x0a3288e4, 0x000c08e4, 'libgpu.cr.so'),
  (0x0a329000, 0x0a3627ec, 0x000397ec, 'libgpu_ipc_service.cr.so'),
  (0x0a363000, 0x0a388a18, 0x00025a18, 'libgpu_util.cr.so'),
  (0x0a389000, 0x0a506d8c, 0x0017dd8c, 'libhost.cr.so'),
  (0x0a507000, 0x0a6f0ec0, 0x001e9ec0, 'libicui18n.cr.so'),
  (0x0a6f1000, 0x0a83b4c8, 0x0014a4c8, 'libicuuc.cr.so'),
  (0x0a83c000, 0x0a8416e4, 0x000056e4, 'libinterfaces_shared.cr.so'),
  (0x0a842000, 0x0a87e2a0, 0x0003c2a0, 'libipc.cr.so'),
  (0x0a87f000, 0x0a88c98c, 0x0000d98c, 'libipc_mojom.cr.so'),
  (0x0a88d000, 0x0a8926e4, 0x000056e4, 'libipc_mojom_shared.cr.so'),
  (0x0a893000, 0x0a8a1e18, 0x0000ee18, 'libkeyed_service_content.cr.so'),
  (0x0a8a2000, 0x0a8b4a30, 0x00012a30, 'libkeyed_service_core.cr.so'),
  (0x0a8b5000, 0x0a930a80, 0x0007ba80, 'libleveldatabase.cr.so'),
  (0x0a931000, 0x0a9b3908, 0x00082908, 'libmanager.cr.so'),
  (0x0a9b4000, 0x0aea9bb4, 0x004f5bb4, 'libmedia.cr.so'),
  (0x0aeaa000, 0x0b08cb88, 0x001e2b88, 'libmedia_blink.cr.so'),
  (0x0b08d000, 0x0b0a4728, 0x00017728, 'libmedia_devices_mojo_bindings_shared.cr.so'),
  (0x0b0a5000, 0x0b1943ec, 0x000ef3ec, 'libmedia_gpu.cr.so'),
  (0x0b195000, 0x0b2d07d4, 0x0013b7d4, 'libmedia_mojo_services.cr.so'),
  (0x0b2d1000, 0x0b2d4760, 0x00003760, 'libmessage_center.cr.so'),
  (0x0b2d5000, 0x0b2e0938, 0x0000b938, 'libmessage_support.cr.so'),
  (0x0b2e1000, 0x0b2f3ad0, 0x00012ad0, 'libmetrics_cpp.cr.so'),
  (0x0b2f4000, 0x0b313bb8, 0x0001fbb8, 'libmidi.cr.so'),
  (0x0b314000, 0x0b31b848, 0x00007848, 'libmojo_base_lib.cr.so'),
  (0x0b31c000, 0x0b3329f8, 0x000169f8, 'libmojo_base_mojom.cr.so'),
  (0x0b333000, 0x0b34b98c, 0x0001898c, 'libmojo_base_mojom_blink.cr.so'),
  (0x0b34c000, 0x0b354700, 0x00008700, 'libmojo_base_mojom_shared.cr.so'),
  (0x0b355000, 0x0b3608b0, 0x0000b8b0, 'libmojo_base_shared_typemap_traits.cr.so'),
  (0x0b361000, 0x0b3ad454, 0x0004c454, 'libmojo_edk.cr.so'),
  (0x0b3ae000, 0x0b3c4a20, 0x00016a20, 'libmojo_edk_ports.cr.so'),
  (0x0b3c5000, 0x0b3d38a0, 0x0000e8a0, 'libmojo_mojom_bindings.cr.so'),
  (0x0b3d4000, 0x0b3da6e8, 0x000066e8, 'libmojo_mojom_bindings_shared.cr.so'),
  (0x0b3db000, 0x0b3e27f0, 0x000077f0, 'libmojo_public_system.cr.so'),
  (0x0b3e3000, 0x0b3fa9fc, 0x000179fc, 'libmojo_public_system_cpp.cr.so'),
  (0x0b3fb000, 0x0b407728, 0x0000c728, 'libmojom_core_shared.cr.so'),
  (0x0b408000, 0x0b421744, 0x00019744, 'libmojom_platform_shared.cr.so'),
  (0x0b422000, 0x0b43451c, 0x0001251c, 'libnative_theme.cr.so'),
  (0x0b435000, 0x0baaa1bc, 0x006751bc, 'libnet.cr.so'),
  (0x0baab000, 0x0bac3c08, 0x00018c08, 'libnet_with_v8.cr.so'),
  (0x0bac4000, 0x0bb74670, 0x000b0670, 'libnetwork_cpp.cr.so'),
  (0x0bb75000, 0x0bbaee8c, 0x00039e8c, 'libnetwork_cpp_base.cr.so'),
  (0x0bbaf000, 0x0bd21844, 0x00172844, 'libnetwork_service.cr.so'),
  (0x0bd22000, 0x0bd256e4, 0x000036e4, 'libnetwork_session_configurator.cr.so'),
  (0x0bd26000, 0x0bd33734, 0x0000d734, 'libonc.cr.so'),
  (0x0bd34000, 0x0bd9ce18, 0x00068e18, 'libperfetto.cr.so'),
  (0x0bd9d000, 0x0bda4854, 0x00007854, 'libplatform.cr.so'),
  (0x0bda5000, 0x0bec5ce4, 0x00120ce4, 'libpolicy_component.cr.so'),
  (0x0bec6000, 0x0bf5ab58, 0x00094b58, 'libpolicy_proto.cr.so'),
  (0x0bf5b000, 0x0bf86fbc, 0x0002bfbc, 'libprefs.cr.so'),
  (0x0bf87000, 0x0bfa5d74, 0x0001ed74, 'libprinting.cr.so'),
  (0x0bfa6000, 0x0bfe0e80, 0x0003ae80, 'libprotobuf_lite.cr.so'),
  (0x0bfe1000, 0x0bff0a18, 0x0000fa18, 'libproxy_config.cr.so'),
  (0x0bff1000, 0x0c0f6654, 0x00105654, 'libpublic.cr.so'),
  (0x0c0f7000, 0x0c0fa6a4, 0x000036a4, 'librange.cr.so'),
  (0x0c0fb000, 0x0c118058, 0x0001d058, 'libraster.cr.so'),
  (0x0c119000, 0x0c133d00, 0x0001ad00, 'libresource_coordinator_cpp.cr.so'),
  (0x0c134000, 0x0c1396a0, 0x000056a0, 'libresource_coordinator_cpp_base.cr.so'),
  (0x0c13a000, 0x0c1973b8, 0x0005d3b8, 'libresource_coordinator_public_mojom.cr.so'),
  (0x0c198000, 0x0c2033e8, 0x0006b3e8, 'libresource_coordinator_public_mojom_blink.cr.so'),
  (0x0c204000, 0x0c219744, 0x00015744, 'libresource_coordinator_public_mojom_shared.cr.so'),
  (0x0c21a000, 0x0c21e700, 0x00004700, 'libsandbox.cr.so'),
  (0x0c21f000, 0x0c22f96c, 0x0001096c, 'libsandbox_services.cr.so'),
  (0x0c230000, 0x0c249d58, 0x00019d58, 'libseccomp_bpf.cr.so'),
  (0x0c24a000, 0x0c24e714, 0x00004714, 'libseccomp_starter_android.cr.so'),
  (0x0c24f000, 0x0c4ae9f0, 0x0025f9f0, 'libservice.cr.so'),
  (0x0c4af000, 0x0c4c3ae4, 0x00014ae4, 'libservice_manager_cpp.cr.so'),
  (0x0c4c4000, 0x0c4cb708, 0x00007708, 'libservice_manager_cpp_types.cr.so'),
  (0x0c4cc000, 0x0c4fbe30, 0x0002fe30, 'libservice_manager_mojom.cr.so'),
  (0x0c4fc000, 0x0c532e78, 0x00036e78, 'libservice_manager_mojom_blink.cr.so'),
  (0x0c533000, 0x0c53669c, 0x0000369c, 'libservice_manager_mojom_constants.cr.so'),
  (0x0c537000, 0x0c53e85c, 0x0000785c, 'libservice_manager_mojom_constants_blink.cr.so'),
  (0x0c53f000, 0x0c542668, 0x00003668, 'libservice_manager_mojom_constants_shared.cr.so'),
  (0x0c543000, 0x0c54d700, 0x0000a700, 'libservice_manager_mojom_shared.cr.so'),
  (0x0c54e000, 0x0c8fc6ec, 0x003ae6ec, 'libsessions.cr.so'),
  (0x0c8fd000, 0x0c90a924, 0x0000d924, 'libshared_memory_support.cr.so'),
  (0x0c90b000, 0x0c9148ec, 0x000098ec, 'libshell_dialogs.cr.so'),
  (0x0c915000, 0x0cf8de70, 0x00678e70, 'libskia.cr.so'),
  (0x0cf8e000, 0x0cf978bc, 0x000098bc, 'libsnapshot.cr.so'),
  (0x0cf98000, 0x0cfb7d9c, 0x0001fd9c, 'libsql.cr.so'),
  (0x0cfb8000, 0x0cfbe744, 0x00006744, 'libstartup_tracing.cr.so'),
  (0x0cfbf000, 0x0d19b4e4, 0x001dc4e4, 'libstorage_browser.cr.so'),
  (0x0d19c000, 0x0d2a773c, 0x0010b73c, 'libstorage_common.cr.so'),
  (0x0d2a8000, 0x0d2ac6fc, 0x000046fc, 'libsurface.cr.so'),
  (0x0d2ad000, 0x0d2baa98, 0x0000da98, 'libtracing.cr.so'),
  (0x0d2bb000, 0x0d2f36b0, 0x000386b0, 'libtracing_cpp.cr.so'),
  (0x0d2f4000, 0x0d326e70, 0x00032e70, 'libtracing_mojom.cr.so'),
  (0x0d327000, 0x0d33270c, 0x0000b70c, 'libtracing_mojom_shared.cr.so'),
  (0x0d333000, 0x0d46d804, 0x0013a804, 'libui_android.cr.so'),
  (0x0d46e000, 0x0d4cb3f8, 0x0005d3f8, 'libui_base.cr.so'),
  (0x0d4cc000, 0x0d4dbc40, 0x0000fc40, 'libui_base_ime.cr.so'),
  (0x0d4dc000, 0x0d4e58d4, 0x000098d4, 'libui_data_pack.cr.so'),
  (0x0d4e6000, 0x0d51d1e0, 0x000371e0, 'libui_devtools.cr.so'),
  (0x0d51e000, 0x0d52b984, 0x0000d984, 'libui_message_center_cpp.cr.so'),
  (0x0d52c000, 0x0d539a48, 0x0000da48, 'libui_touch_selection.cr.so'),
  (0x0d53a000, 0x0d55bc60, 0x00021c60, 'liburl.cr.so'),
  (0x0d55c000, 0x0d55f6b4, 0x000036b4, 'liburl_ipc.cr.so'),
  (0x0d560000, 0x0d5af110, 0x0004f110, 'liburl_matcher.cr.so'),
  (0x0d5b0000, 0x0d5e2fac, 0x00032fac, 'libuser_manager.cr.so'),
  (0x0d5e3000, 0x0d5e66e4, 0x000036e4, 'libuser_prefs.cr.so'),
  (0x0d5e7000, 0x0e3e1cc8, 0x00dfacc8, 'libv8.cr.so'),
  (0x0e3e2000, 0x0e400ae0, 0x0001eae0, 'libv8_libbase.cr.so'),
  (0x0e401000, 0x0e4d91d4, 0x000d81d4, 'libviz_common.cr.so'),
  (0x0e4da000, 0x0e4df7e4, 0x000057e4, 'libviz_resource_format.cr.so'),
  (0x0e4e0000, 0x0e5b7120, 0x000d7120, 'libweb_dialogs.cr.so'),
  (0x0e5b8000, 0x0e5c7a18, 0x0000fa18, 'libwebdata_common.cr.so'),
  (0x0e5c8000, 0x0e61bfe4, 0x00053fe4, 'libwtf.cr.so'),
]


# A small memory map fragment extracted from a tombstone for a process that
# had loaded the APK corresponding to _TEST_APK_LIBS above.
_TEST_MEMORY_MAP = r'''memory map:
12c00000-12ccafff rw-         0     cb000  /dev/ashmem/dalvik-main space (deleted)
12ccb000-130cafff rw-     cb000    400000  /dev/ashmem/dalvik-main space (deleted)
130cb000-32bfffff ---    4cb000  1fb35000  /dev/ashmem/dalvik-main space (deleted)
32c00000-32c00fff rw-         0      1000  /dev/ashmem/dalvik-main space 1 (deleted)
32c01000-52bfffff ---      1000  1ffff000  /dev/ashmem/dalvik-main space 1 (deleted)
6f3b8000-6fd90fff rw-         0    9d9000  /data/dalvik-cache/x86/system@framework@boot.art
6fd91000-71c42fff r--         0   1eb2000  /data/dalvik-cache/x86/system@framework@boot.oat
71c43000-7393efff r-x   1eb2000   1cfc000  /data/dalvik-cache/x86/system@framework@boot.oat (load base 0x71c43000)
7393f000-7393ffff rw-   3bae000      1000  /data/dalvik-cache/x86/system@framework@boot.oat
73940000-73a1bfff rw-         0     dc000  /dev/ashmem/dalvik-zygote space (deleted)
73a1c000-73a1cfff rw-         0      1000  /dev/ashmem/dalvik-non moving space (deleted)
73a1d000-73a2dfff rw-      1000     11000  /dev/ashmem/dalvik-non moving space (deleted)
73a2e000-77540fff ---     12000   3b13000  /dev/ashmem/dalvik-non moving space (deleted)
77541000-7793ffff rw-   3b25000    3ff000  /dev/ashmem/dalvik-non moving space (deleted)
923aa000-92538fff r--    8a9000    18f000  /data/app/com.example.app-2/base.apk
92539000-9255bfff r--         0     23000  /data/data/com.example.app/app_data/paks/es.pak@162db1c6689
9255c000-92593fff r--    213000     38000  /data/app/com.example.app-2/base.apk
92594000-925c0fff r--    87d000     2d000  /data/app/com.example.app-2/base.apk
925c1000-927d3fff r--    a37000    213000  /data/app/com.example.app-2/base.apk
927d4000-92e07fff r--    24a000    634000  /data/app/com.example.app-2/base.apk
92e08000-92e37fff r--   a931000     30000  /data/app/com.example.app-2/base.apk
92e38000-92e86fff r-x   a961000     4f000  /data/app/com.example.app-2/base.apk
92e87000-92e8afff rw-   a9b0000      4000  /data/app/com.example.app-2/base.apk
92e8b000-92e8bfff rw-         0      1000
92e8c000-92e9dfff r--   d5b0000     12000  /data/app/com.example.app-2/base.apk
92e9e000-92ebcfff r-x   d5c2000     1f000  /data/app/com.example.app-2/base.apk
92ebd000-92ebefff rw-   d5e1000      2000  /data/app/com.example.app-2/base.apk
92ebf000-92ebffff rw-         0      1000
'''

# list of (address, size, path, offset)  tuples that must appear in
# _TEST_MEMORY_MAP. Not all sections need to be listed.
_TEST_MEMORY_MAP_SECTIONS = [
  (0x923aa000, 0x18f000, '/data/app/com.example.app-2/base.apk', 0x8a9000),
  (0x9255c000, 0x038000, '/data/app/com.example.app-2/base.apk', 0x213000),
  (0x92594000, 0x02d000, '/data/app/com.example.app-2/base.apk', 0x87d000),
  (0x925c1000, 0x213000, '/data/app/com.example.app-2/base.apk', 0xa37000),
]

_EXPECTED_TEST_MEMORY_MAP = r'''memory map:
12c00000-12ccafff rw-         0     cb000  /dev/ashmem/dalvik-main space (deleted)
12ccb000-130cafff rw-     cb000    400000  /dev/ashmem/dalvik-main space (deleted)
130cb000-32bfffff ---    4cb000  1fb35000  /dev/ashmem/dalvik-main space (deleted)
32c00000-32c00fff rw-         0      1000  /dev/ashmem/dalvik-main space 1 (deleted)
32c01000-52bfffff ---      1000  1ffff000  /dev/ashmem/dalvik-main space 1 (deleted)
6f3b8000-6fd90fff rw-         0    9d9000  /data/dalvik-cache/x86/system@framework@boot.art
6fd91000-71c42fff r--         0   1eb2000  /data/dalvik-cache/x86/system@framework@boot.oat
71c43000-7393efff r-x   1eb2000   1cfc000  /data/dalvik-cache/x86/system@framework@boot.oat (load base 0x71c43000)
7393f000-7393ffff rw-   3bae000      1000  /data/dalvik-cache/x86/system@framework@boot.oat
73940000-73a1bfff rw-         0     dc000  /dev/ashmem/dalvik-zygote space (deleted)
73a1c000-73a1cfff rw-         0      1000  /dev/ashmem/dalvik-non moving space (deleted)
73a1d000-73a2dfff rw-      1000     11000  /dev/ashmem/dalvik-non moving space (deleted)
73a2e000-77540fff ---     12000   3b13000  /dev/ashmem/dalvik-non moving space (deleted)
77541000-7793ffff rw-   3b25000    3ff000  /dev/ashmem/dalvik-non moving space (deleted)
923aa000-92538fff r--    8a9000    18f000  /data/app/com.example.app-2/base.apk
92539000-9255bfff r--         0     23000  /data/data/com.example.app/app_data/paks/es.pak@162db1c6689
9255c000-92593fff r--    213000     38000  /data/app/com.example.app-2/base.apk
92594000-925c0fff r--    87d000     2d000  /data/app/com.example.app-2/base.apk
925c1000-927d3fff r--    a37000    213000  /data/app/com.example.app-2/base.apk
927d4000-92e07fff r--    24a000    634000  /data/app/com.example.app-2/base.apk
92e08000-92e37fff r--   a931000     30000  /data/app/com.example.app-2/base.apk!lib/libmanager.cr.so (offset 0x0)
92e38000-92e86fff r-x   a961000     4f000  /data/app/com.example.app-2/base.apk!lib/libmanager.cr.so (offset 0x30000)
92e87000-92e8afff rw-   a9b0000      4000  /data/app/com.example.app-2/base.apk!lib/libmanager.cr.so (offset 0x7f000)
92e8b000-92e8bfff rw-         0      1000
92e8c000-92e9dfff r--   d5b0000     12000  /data/app/com.example.app-2/base.apk!lib/libuser_manager.cr.so (offset 0x0)
92e9e000-92ebcfff r-x   d5c2000     1f000  /data/app/com.example.app-2/base.apk!lib/libuser_manager.cr.so (offset 0x12000)
92ebd000-92ebefff rw-   d5e1000      2000  /data/app/com.example.app-2/base.apk!lib/libuser_manager.cr.so (offset 0x31000)
92ebf000-92ebffff rw-         0      1000
'''

# Example stack section, taken from the same tombstone that _TEST_MEMORY_MAP
# was extracted from.
_TEST_STACK = r'''stack:
        bf89a070  b7439468  /system/lib/libc.so
        bf89a074  bf89a1e4  [stack]
        bf89a078  932d4000  /data/app/com.example.app-2/base.apk
        bf89a07c  b73bfbc9  /system/lib/libc.so (pthread_mutex_lock+65)
        bf89a080  00000000
        bf89a084  4000671c  /dev/ashmem/dalvik-main space 1 (deleted)
        bf89a088  932d1d86  /data/app/com.example.app-2/base.apk
        bf89a08c  b743671c  /system/lib/libc.so
        bf89a090  b77f8c00  /system/bin/linker
        bf89a094  b743cc90
        bf89a098  932d1d4a  /data/app/com.example.app-2/base.apk
        bf89a09c  b73bf271  /system/lib/libc.so (__pthread_internal_find(long)+65)
        bf89a0a0  b743cc90
        bf89a0a4  bf89a0b0  [stack]
        bf89a0a8  bf89a0b8  [stack]
        bf89a0ac  00000008
        ........  ........
  #00  bf89a0b0  00000006
        bf89a0b4  00000002
        bf89a0b8  b743671c  /system/lib/libc.so
        bf89a0bc  b73bf5d9  /system/lib/libc.so (pthread_kill+71)
  #01  bf89a0c0  00006937
        bf89a0c4  00006937
        bf89a0c8  00000006
        bf89a0cc  b77fd3a9  /system/bin/app_process32 (sigprocmask+141)
        bf89a0d0  00000002
        bf89a0d4  bf89a0ec  [stack]
        bf89a0d8  00000000
        bf89a0dc  b743671c  /system/lib/libc.so
        bf89a0e0  bf89a12c  [stack]
        bf89a0e4  bf89a1e4  [stack]
        bf89a0e8  932d1d4a  /data/app/com.example.app-2/base.apk
        bf89a0ec  b7365206  /system/lib/libc.so (raise+37)
  #02  bf89a0f0  b77f8c00  /system/bin/linker
        bf89a0f4  00000006
        bf89a0f8  b7439468  /system/lib/libc.so
        bf89a0fc  b743671c  /system/lib/libc.so
        bf89a100  bf89a12c  [stack]
        bf89a104  b743671c  /system/lib/libc.so
        bf89a108  bf89a12c  [stack]
        bf89a10c  b735e9e5  /system/lib/libc.so (abort+81)
  #03  bf89a110  00000006
        bf89a114  bf89a12c  [stack]
        bf89a118  00000000
        bf89a11c  b55a3d3b  /system/lib/libprotobuf-cpp-lite.so (google::protobuf::internal::DefaultLogHandler(google::protobuf::LogLevel, char const*, int, std::__1::basic_string<char, std::__1::char_traits<char>, std::__1::allocator<char> > const&)+99)
        bf89a120  b7439468  /system/lib/libc.so
        bf89a124  b55ba38d  /system/lib/libprotobuf-cpp-lite.so
        bf89a128  b55ba408  /system/lib/libprotobuf-cpp-lite.so
        bf89a12c  ffffffdf
        bf89a130  0000003d
        bf89a134  adfedf00  [anon:libc_malloc]
        bf89a138  bf89a158  [stack]
  #04  bf89a13c  a0cee7f0  /data/app/com.example.app-2/base.apk
        bf89a140  b55c1cb0  /system/lib/libprotobuf-cpp-lite.so
        bf89a144  bf89a1e4  [stack]
'''

# Expected value of _TEST_STACK after translation of addresses in the APK
# into offsets into libraries.
_EXPECTED_STACK = r'''stack:
        bf89a070  b7439468  /system/lib/libc.so
        bf89a074  bf89a1e4  [stack]
        bf89a078  932d4000  /data/app/com.example.app-2/base.apk
        bf89a07c  b73bfbc9  /system/lib/libc.so (pthread_mutex_lock+65)
        bf89a080  00000000
        bf89a084  4000671c  /dev/ashmem/dalvik-main space 1 (deleted)
        bf89a088  932d1d86  /data/app/com.example.app-2/base.apk
        bf89a08c  b743671c  /system/lib/libc.so
        bf89a090  b77f8c00  /system/bin/linker
        bf89a094  b743cc90
        bf89a098  932d1d4a  /data/app/com.example.app-2/base.apk
        bf89a09c  b73bf271  /system/lib/libc.so (__pthread_internal_find(long)+65)
        bf89a0a0  b743cc90
        bf89a0a4  bf89a0b0  [stack]
        bf89a0a8  bf89a0b8  [stack]
        bf89a0ac  00000008
        ........  ........
  #00  bf89a0b0  00000006
        bf89a0b4  00000002
        bf89a0b8  b743671c  /system/lib/libc.so
        bf89a0bc  b73bf5d9  /system/lib/libc.so (pthread_kill+71)
  #01  bf89a0c0  00006937
        bf89a0c4  00006937
        bf89a0c8  00000006
        bf89a0cc  b77fd3a9  /system/bin/app_process32 (sigprocmask+141)
        bf89a0d0  00000002
        bf89a0d4  bf89a0ec  [stack]
        bf89a0d8  00000000
        bf89a0dc  b743671c  /system/lib/libc.so
        bf89a0e0  bf89a12c  [stack]
        bf89a0e4  bf89a1e4  [stack]
        bf89a0e8  932d1d4a  /data/app/com.example.app-2/base.apk
        bf89a0ec  b7365206  /system/lib/libc.so (raise+37)
  #02  bf89a0f0  b77f8c00  /system/bin/linker
        bf89a0f4  00000006
        bf89a0f8  b7439468  /system/lib/libc.so
        bf89a0fc  b743671c  /system/lib/libc.so
        bf89a100  bf89a12c  [stack]
        bf89a104  b743671c  /system/lib/libc.so
        bf89a108  bf89a12c  [stack]
        bf89a10c  b735e9e5  /system/lib/libc.so (abort+81)
  #03  bf89a110  00000006
        bf89a114  bf89a12c  [stack]
        bf89a118  00000000
        bf89a11c  b55a3d3b  /system/lib/libprotobuf-cpp-lite.so (google::protobuf::internal::DefaultLogHandler(google::protobuf::LogLevel, char const*, int, std::__1::basic_string<char, std::__1::char_traits<char>, std::__1::allocator<char> > const&)+99)
        bf89a120  b7439468  /system/lib/libc.so
        bf89a124  b55ba38d  /system/lib/libprotobuf-cpp-lite.so
        bf89a128  b55ba408  /system/lib/libprotobuf-cpp-lite.so
        bf89a12c  ffffffdf
        bf89a130  0000003d
        bf89a134  adfedf00  [anon:libc_malloc]
        bf89a138  bf89a158  [stack]
  #04  bf89a13c  a0cee7f0  /data/app/com.example.app-2/base.apk
        bf89a140  b55c1cb0  /system/lib/libprotobuf-cpp-lite.so
        bf89a144  bf89a1e4  [stack]
'''

_TEST_BACKTRACE = r'''backtrace:
    #00 pc 00084126  /system/lib/libc.so (tgkill+22)
    #01 pc 000815d8  /system/lib/libc.so (pthread_kill+70)
    #02 pc 00027205  /system/lib/libc.so (raise+36)
    #03 pc 000209e4  /system/lib/libc.so (abort+80)
    #04 pc 0000cf73  /system/lib/libprotobuf-cpp-lite.so (google::protobuf::internal::LogMessage::Finish()+117)
    #05 pc 0000cf8e  /system/lib/libprotobuf-cpp-lite.so (google::protobuf::internal::LogFinisher::operator=(google::protobuf::internal::LogMessage&)+26)
    #06 pc 0000d27f  /system/lib/libprotobuf-cpp-lite.so (google::protobuf::internal::VerifyVersion(int, int, char const*)+574)
    #07 pc 007cd236  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #08 pc 000111a9  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0xbfc2000)
    #09 pc 00013228  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0xbfc2000)
    #10 pc 000131de  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0xbfc2000)
    #11 pc 007cd2d8  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #12 pc 007cd956  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #13 pc 007c2d4a  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #14 pc 009fc9f1  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #15 pc 009fc8ea  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #16 pc 00561c63  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #17 pc 0106fbdb  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #18 pc 004d7371  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #19 pc 004d8159  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #20 pc 004d7b96  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #21 pc 004da4b6  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #22 pc 005ab66c  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x7daa000)
    #23 pc 005afca2  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x7daa000)
    #24 pc 0000cae8  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x598d000)
    #25 pc 00ce864f  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x7daa000)
    #26 pc 00ce8dfa  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x7daa000)
    #27 pc 00ce74c6  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x7daa000)
    #28 pc 00004616  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x961e000)
    #29 pc 00ce8215  /data/app/com.google.android.apps.chrome-2/base.apk (offset 0x7daa000)
    #30 pc 0013d8c7  /system/lib/libart.so (art_quick_generic_jni_trampoline+71)
    #31 pc 00137c52  /system/lib/libart.so (art_quick_invoke_static_stub+418)
    #32 pc 00143651  /system/lib/libart.so (art::ArtMethod::Invoke(art::Thread*, unsigned int*, unsigned int, art::JValue*, char const*)+353)
    #33 pc 005e06ae  /system/lib/libart.so (artInterpreterToCompiledCodeBridge+190)
    #34 pc 00328b5d  /system/lib/libart.so (bool art::interpreter::DoCall<false, false>(art::ArtMethod*, art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+445)
    #35 pc 0032cfc0  /system/lib/libart.so (bool art::interpreter::DoInvoke<(art::InvokeType)0, false, false>(art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+160)
    #36 pc 000fc703  /system/lib/libart.so (art::JValue art::interpreter::ExecuteGotoImpl<false, false>(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame&, art::JValue)+29891)
    #37 pc 00300af7  /system/lib/libart.so (artInterpreterToInterpreterBridge+188)
    #38 pc 00328b5d  /system/lib/libart.so (bool art::interpreter::DoCall<false, false>(art::ArtMethod*, art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+445)
    #39 pc 0032cfc0  /system/lib/libart.so (bool art::interpreter::DoInvoke<(art::InvokeType)0, false, false>(art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+160)
    #40 pc 000fc703  /system/lib/libart.so (art::JValue art::interpreter::ExecuteGotoImpl<false, false>(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame&, art::JValue)+29891)
    #41 pc 00300af7  /system/lib/libart.so (artInterpreterToInterpreterBridge+188)
    #42 pc 00328b5d  /system/lib/libart.so (bool art::interpreter::DoCall<false, false>(art::ArtMethod*, art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+445)
    #43 pc 0032ebf9  /system/lib/libart.so (bool art::interpreter::DoInvoke<(art::InvokeType)2, false, false>(art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+297)
    #44 pc 000fc955  /system/lib/libart.so (art::JValue art::interpreter::ExecuteGotoImpl<false, false>(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame&, art::JValue)+30485)
    #45 pc 00300af7  /system/lib/libart.so (artInterpreterToInterpreterBridge+188)
    #46 pc 00328b5d  /system/lib/libart.so (bool art::interpreter::DoCall<false, false>(art::ArtMethod*, art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+445)
    #47 pc 0033090c  /system/lib/libart.so (bool art::interpreter::DoInvoke<(art::InvokeType)4, false, false>(art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+636)
    #48 pc 000fc67f  /system/lib/libart.so (art::JValue art::interpreter::ExecuteGotoImpl<false, false>(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame&, art::JValue)+29759)
    #49 pc 00300700  /system/lib/libart.so (art::interpreter::EnterInterpreterFromEntryPoint(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame*)+128)
    #50 pc 00667c73  /system/lib/libart.so (artQuickToInterpreterBridge+808)
    #51 pc 0013d98d  /system/lib/libart.so (art_quick_to_interpreter_bridge+77)
    #52 pc 7264bc5b  /data/dalvik-cache/x86/system@framework@boot.oat (offset 0x1eb2000)
'''

_EXPECTED_BACKTRACE = r'''backtrace:
    #00 pc 00084126  /system/lib/libc.so (tgkill+22)
    #01 pc 000815d8  /system/lib/libc.so (pthread_kill+70)
    #02 pc 00027205  /system/lib/libc.so (raise+36)
    #03 pc 000209e4  /system/lib/libc.so (abort+80)
    #04 pc 0000cf73  /system/lib/libprotobuf-cpp-lite.so (google::protobuf::internal::LogMessage::Finish()+117)
    #05 pc 0000cf8e  /system/lib/libprotobuf-cpp-lite.so (google::protobuf::internal::LogFinisher::operator=(google::protobuf::internal::LogMessage&)+26)
    #06 pc 0000d27f  /system/lib/libprotobuf-cpp-lite.so (google::protobuf::internal::VerifyVersion(int, int, char const*)+574)
    #07 pc 007cd236  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #08 pc 000111a9  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libprotobuf_lite.cr.so (offset 0x1c000)
    #09 pc 00013228  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libprotobuf_lite.cr.so (offset 0x1c000)
    #10 pc 000131de  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libprotobuf_lite.cr.so (offset 0x1c000)
    #11 pc 007cd2d8  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #12 pc 007cd956  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #13 pc 007c2d4a  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #14 pc 009fc9f1  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #15 pc 009fc8ea  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #16 pc 00561c63  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #17 pc 0106fbdb  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #18 pc 004d7371  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #19 pc 004d8159  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #20 pc 004d7b96  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #21 pc 004da4b6  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #22 pc 005ab66c  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libcontent.cr.so (offset 0xc2d000)
    #23 pc 005afca2  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libcontent.cr.so (offset 0xc2d000)
    #24 pc 0000cae8  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so (offset 0x90e000)
    #25 pc 00ce864f  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libcontent.cr.so (offset 0xc2d000)
    #26 pc 00ce8dfa  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libcontent.cr.so (offset 0xc2d000)
    #27 pc 00ce74c6  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libcontent.cr.so (offset 0xc2d000)
    #28 pc 00004616  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libembedder.cr.so (offset 0x28000)
    #29 pc 00ce8215  /data/app/com.google.android.apps.chrome-2/base.apk!lib/libcontent.cr.so (offset 0xc2d000)
    #30 pc 0013d8c7  /system/lib/libart.so (art_quick_generic_jni_trampoline+71)
    #31 pc 00137c52  /system/lib/libart.so (art_quick_invoke_static_stub+418)
    #32 pc 00143651  /system/lib/libart.so (art::ArtMethod::Invoke(art::Thread*, unsigned int*, unsigned int, art::JValue*, char const*)+353)
    #33 pc 005e06ae  /system/lib/libart.so (artInterpreterToCompiledCodeBridge+190)
    #34 pc 00328b5d  /system/lib/libart.so (bool art::interpreter::DoCall<false, false>(art::ArtMethod*, art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+445)
    #35 pc 0032cfc0  /system/lib/libart.so (bool art::interpreter::DoInvoke<(art::InvokeType)0, false, false>(art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+160)
    #36 pc 000fc703  /system/lib/libart.so (art::JValue art::interpreter::ExecuteGotoImpl<false, false>(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame&, art::JValue)+29891)
    #37 pc 00300af7  /system/lib/libart.so (artInterpreterToInterpreterBridge+188)
    #38 pc 00328b5d  /system/lib/libart.so (bool art::interpreter::DoCall<false, false>(art::ArtMethod*, art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+445)
    #39 pc 0032cfc0  /system/lib/libart.so (bool art::interpreter::DoInvoke<(art::InvokeType)0, false, false>(art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+160)
    #40 pc 000fc703  /system/lib/libart.so (art::JValue art::interpreter::ExecuteGotoImpl<false, false>(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame&, art::JValue)+29891)
    #41 pc 00300af7  /system/lib/libart.so (artInterpreterToInterpreterBridge+188)
    #42 pc 00328b5d  /system/lib/libart.so (bool art::interpreter::DoCall<false, false>(art::ArtMethod*, art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+445)
    #43 pc 0032ebf9  /system/lib/libart.so (bool art::interpreter::DoInvoke<(art::InvokeType)2, false, false>(art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+297)
    #44 pc 000fc955  /system/lib/libart.so (art::JValue art::interpreter::ExecuteGotoImpl<false, false>(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame&, art::JValue)+30485)
    #45 pc 00300af7  /system/lib/libart.so (artInterpreterToInterpreterBridge+188)
    #46 pc 00328b5d  /system/lib/libart.so (bool art::interpreter::DoCall<false, false>(art::ArtMethod*, art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+445)
    #47 pc 0033090c  /system/lib/libart.so (bool art::interpreter::DoInvoke<(art::InvokeType)4, false, false>(art::Thread*, art::ShadowFrame&, art::Instruction const*, unsigned short, art::JValue*)+636)
    #48 pc 000fc67f  /system/lib/libart.so (art::JValue art::interpreter::ExecuteGotoImpl<false, false>(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame&, art::JValue)+29759)
    #49 pc 00300700  /system/lib/libart.so (art::interpreter::EnterInterpreterFromEntryPoint(art::Thread*, art::DexFile::CodeItem const*, art::ShadowFrame*)+128)
    #50 pc 00667c73  /system/lib/libart.so (artQuickToInterpreterBridge+808)
    #51 pc 0013d98d  /system/lib/libart.so (art_quick_to_interpreter_bridge+77)
    #52 pc 7264bc5b  /data/dalvik-cache/x86/system@framework@boot.oat (offset 0x1eb2000)
'''

_EXPECTED_BACKTRACE_OFFSETS_MAP = {
  '/data/app/com.google.android.apps.chrome-2/base.apk!lib/libprotobuf_lite.cr.so':
      set([
          0x1c000 + 0x111a9,
          0x1c000 + 0x13228,
          0x1c000 + 0x131de,
      ]),

  '/data/app/com.google.android.apps.chrome-2/base.apk!lib/libchrome.cr.so':
      set([
          0x90e000 + 0x7cd236,
          0x90e000 + 0x7cd2d8,
          0x90e000 + 0x7cd956,
          0x90e000 + 0x7c2d4a,
          0x90e000 + 0x9fc9f1,
          0x90e000 + 0x9fc8ea,
          0x90e000 + 0x561c63,
          0x90e000 + 0x106fbdb,
          0x90e000 + 0x4d7371,
          0x90e000 + 0x4d8159,
          0x90e000 + 0x4d7b96,
          0x90e000 + 0x4da4b6,
          0x90e000 + 0xcae8,
      ]),
  '/data/app/com.google.android.apps.chrome-2/base.apk!lib/libcontent.cr.so':
      set([
          0xc2d000 + 0x5ab66c,
          0xc2d000 + 0x5afca2,
          0xc2d000 + 0xce864f,
          0xc2d000 + 0xce8dfa,
          0xc2d000 + 0xce74c6,
          0xc2d000 + 0xce8215,
      ]),
  '/data/app/com.google.android.apps.chrome-2/base.apk!lib/libembedder.cr.so':
      set([
          0x28000 + 0x4616,
      ])
}

# pylint: enable=line-too-long

_ONE_MB = 1024 * 1024
_TEST_SYMBOL_DATA = {
  # Regular symbols
  0: 'mock_sym_for_addr_0 [mock_src/libmock1.so.c:0]',
  0x1000: 'mock_sym_for_addr_4096 [mock_src/libmock1.so.c:4096]',

  # Symbols without source file path.
  _ONE_MB: 'mock_sym_for_addr_1048576 [??:0]',
  _ONE_MB + 0x8234: 'mock_sym_for_addr_1081908 [??:0]',

  # Unknown symbol.
  2 * _ONE_MB: '?? [??:0]',

  # Inlined symbol.
  3 * _ONE_MB:
    'mock_sym_for_addr_3145728_inner [mock_src/libmock1.so.c:3145728]',
}

@contextlib.contextmanager
def _TempDir():
  dirname = tempfile.mkdtemp()
  try:
    yield dirname
  finally:
    shutil.rmtree(dirname)


def _TouchFile(path):
  # Create parent directories.
  try:
    os.makedirs(os.path.dirname(path))
  except OSError:
    pass
  with open(path, 'a'):
    os.utime(path, None)

class MockApkTranslator(object):
  """A mock ApkLibraryPathTranslator object used for testing."""

  # Regex that matches the content of APK native library map files generated
  # with apk_lib_dump.py.
  _RE_MAP_FILE = re.compile(
      r'0x(?P<file_start>[0-9a-f]+)\s+' +
      r'0x(?P<file_end>[0-9a-f]+)\s+' +
      r'0x(?P<file_size>[0-9a-f]+)\s+' +
      r'0x(?P<lib_path>[0-9a-f]+)\s+')

  def __init__(self, test_apk_libs=None):
    """Initialize instance.

    Args:
      test_apk_libs: Optional list of (file_start, file_end, size, lib_path)
        tuples, like _TEST_APK_LIBS for example. This will be used to
        implement TranslatePath().
    """
    self._apk_libs = []
    if test_apk_libs:
      self._AddLibEntries(test_apk_libs)

  def _AddLibEntries(self, entries):
    self._apk_libs = sorted(self._apk_libs + entries,
                            lambda x, y: cmp(x[0], y[0]))

  def ReadMapFile(self, file_path):
    """Read an .apk.native-libs file that was produced with apk_lib_dump.py.

    Args:
      file_path: input path to .apk.native-libs file. Its format is
        essentially: 0x<start>  0x<end> 0x<size> <library-path>
    """
    new_libs = []
    with open(file_path) as f:
      for line in f.readlines():
        m = MockApkTranslator._RE_MAP_FILE.match(line)
        if m:
          file_start = int(m.group('file_start'), 16)
          file_end = int(m.group('file_end'), 16)
          file_size = int(m.group('file_size'), 16)
          lib_path = m.group('lib_path')
          # Sanity check
          if file_start + file_size != file_end:
            logging.warning('%s: Inconsistent (start, end, size) values '
                            '(0x%x, 0x%x, 0x%x)',
                            file_path, file_start, file_end, file_size)
          else:
            new_libs.append((file_start, file_end, file_size, lib_path))

    self._AddLibEntries(new_libs)

  def TranslatePath(self, lib_path, lib_offset):
    """Translate an APK file path + offset into a library path + offset."""
    min_pos = 0
    max_pos = len(self._apk_libs)
    while min_pos < max_pos:
      mid_pos = (min_pos + max_pos) / 2
      mid_entry = self._apk_libs[mid_pos]
      mid_offset = mid_entry[0]
      mid_size = mid_entry[2]
      if lib_offset < mid_offset:
        max_pos = mid_pos
      elif lib_offset >= mid_offset + mid_size:
        min_pos = mid_pos + 1
      else:
        # Found it
        new_path = '%s!lib/%s' % (lib_path, mid_entry[3])
        new_offset = lib_offset - mid_offset
        return (new_path, new_offset)

    return lib_path, lib_offset


class HostLibraryFinderTest(unittest.TestCase):

  def testEmpty(self):
    finder = symbol_utils.HostLibraryFinder()
    self.assertIsNone(finder.Find('/data/data/com.example.app-1/lib/libfoo.so'))
    self.assertIsNone(
        finder.Find('/data/data/com.example.app-1/base.apk!lib/libfoo.so'))


  def testSimpleDirectory(self):
    finder = symbol_utils.HostLibraryFinder()
    with _TempDir() as tmp_dir:
      host_libfoo_path = os.path.join(tmp_dir, 'libfoo.so')
      host_libbar_path = os.path.join(tmp_dir, 'libbar.so')
      _TouchFile(host_libfoo_path)
      _TouchFile(host_libbar_path)

      finder.AddSearchDir(tmp_dir)

      # Regular library path (extracted at installation by the PackageManager).
      # Note that the extraction path has changed between Android releases,
      # i.e. it can be /data/app/, /data/data/ or /data/app-lib/ depending
      # on the system.
      self.assertEqual(
          host_libfoo_path,
          finder.Find('/data/app-lib/com.example.app-1/lib/libfoo.so'))

      # Verify that the path doesn't really matter
      self.assertEqual(
          host_libfoo_path,
          finder.Find('/whatever/what.apk!lib/libfoo.so'))

      self.assertEqual(
          host_libbar_path,
          finder.Find('/data/data/com.example.app-1/lib/libbar.so'))

      self.assertIsNone(
          finder.Find('/data/data/com.example.app-1/lib/libunknown.so'))


  def testMultipleDirectories(self):
    with _TempDir() as tmp_dir:
      # Create the following files:
      #   <tmp_dir>/aaa/
      #      libfoo.so
      #   <tmp_dir>/bbb/
      #      libbar.so
      #      libfoo.so    (this one should never be seen because 'aaa'
      #                    shall be first in the search path list).
      #
      aaa_dir = os.path.join(tmp_dir, 'aaa')
      bbb_dir = os.path.join(tmp_dir, 'bbb')
      os.makedirs(aaa_dir)
      os.makedirs(bbb_dir)

      host_libfoo_path = os.path.join(aaa_dir, 'libfoo.so')
      host_libbar_path = os.path.join(bbb_dir, 'libbar.so')
      host_libfoo2_path = os.path.join(bbb_dir, 'libfoo.so')

      _TouchFile(host_libfoo_path)
      _TouchFile(host_libbar_path)
      _TouchFile(host_libfoo2_path)

      finder = symbol_utils.HostLibraryFinder()
      finder.AddSearchDir(aaa_dir)
      finder.AddSearchDir(bbb_dir)

      self.assertEqual(
          host_libfoo_path,
          finder.Find('/data/data/com.example.app-1/lib/libfoo.so'))

      self.assertEqual(
          host_libfoo_path,
          finder.Find('/data/whatever/base.apk!lib/libfoo.so'))

      self.assertEqual(
          host_libbar_path,
          finder.Find('/data/data/com.example.app-1/lib/libbar.so'))

      self.assertIsNone(
          finder.Find('/data/data/com.example.app-1/lib/libunknown.so'))


class ElfSymbolResolverTest(unittest.TestCase):

  def testCreation(self):
    resolver = symbol_utils.ElfSymbolResolver(
        addr2line_path_for_tests=_MOCK_A2L_PATH)
    self.assertTrue(resolver)

  def testWithSimpleOffsets(self):
    resolver = symbol_utils.ElfSymbolResolver(
        addr2line_path_for_tests=_MOCK_A2L_PATH)
    resolver.SetAndroidAbi('ignored-abi')

    for addr, expected_sym in _TEST_SYMBOL_DATA.iteritems():
      self.assertEqual(resolver.FindSymbolInfo('/some/path/libmock1.so', addr),
                       expected_sym)

  def testWithPreResolvedSymbols(self):
    resolver = symbol_utils.ElfSymbolResolver(
        addr2line_path_for_tests=_MOCK_A2L_PATH)
    resolver.SetAndroidAbi('ignored-abi')
    resolver.AddLibraryOffsets('/some/path/libmock1.so',
                               _TEST_SYMBOL_DATA.keys())

    resolver.DisallowSymbolizerForTesting()

    for addr, expected_sym in _TEST_SYMBOL_DATA.iteritems():
      sym_info = resolver.FindSymbolInfo('/some/path/libmock1.so', addr)
      self.assertIsNotNone(sym_info, 'None symbol info for addr %x' % addr)
      self.assertEqual(
          sym_info, expected_sym,
          'Invalid symbol info for addr %x [%s] expected [%s]' % (
              addr, sym_info, expected_sym))


class MemoryMapTest(unittest.TestCase):

  def testCreation(self):
    mem_map = symbol_utils.MemoryMap('test-abi32')
    self.assertIsNone(mem_map.FindSectionForAddress(0))

  def testParseLines(self):
    mem_map = symbol_utils.MemoryMap('test-abi32')
    mem_map.ParseLines(_TEST_MEMORY_MAP.splitlines())
    for exp_addr, exp_size, exp_path, exp_offset in _TEST_MEMORY_MAP_SECTIONS:
      text = '(addr:%x, size:%x, path:%s, offset=%x)' % (
          exp_addr, exp_size, exp_path, exp_offset)

      t = mem_map.FindSectionForAddress(exp_addr)
      self.assertTrue(t, 'Could not find %s' % text)
      self.assertEqual(t.address, exp_addr)
      self.assertEqual(t.size, exp_size)
      self.assertEqual(t.offset, exp_offset)
      self.assertEqual(t.path, exp_path)

  def testTranslateLine(self):
    android_abi = 'test-abi'
    apk_translator = MockApkTranslator(_TEST_APK_LIBS)
    mem_map = symbol_utils.MemoryMap(android_abi)
    for line, expected_line in zip(_TEST_MEMORY_MAP.splitlines(),
                                   _EXPECTED_TEST_MEMORY_MAP.splitlines()):
      self.assertEqual(mem_map.TranslateLine(line, apk_translator),
                       expected_line)

class StackTranslatorTest(unittest.TestCase):

  def testSimpleStack(self):
    android_abi = 'test-abi32'
    mem_map = symbol_utils.MemoryMap(android_abi)
    mem_map.ParseLines(_TEST_MEMORY_MAP)
    apk_translator = MockApkTranslator(_TEST_APK_LIBS)
    stack_translator = symbol_utils.StackTranslator(android_abi, mem_map,
                                                    apk_translator)
    input_stack = _TEST_STACK.splitlines()
    expected_stack = _EXPECTED_STACK.splitlines()
    self.assertEqual(len(input_stack), len(expected_stack))
    for stack_line, expected_line in zip(input_stack, expected_stack):
      new_line = stack_translator.TranslateLine(stack_line)
      self.assertEqual(new_line, expected_line)


class MockSymbolResolver(symbol_utils.SymbolResolver):

  # A regex matching a symbol definition as it appears in a test symbol file.
  # Format is: <hex-offset> <whitespace> <symbol-string>
  _RE_SYMBOL_DEFINITION = re.compile(
      r'(?P<offset>[0-9a-f]+)\s+(?P<symbol>.*)')

  def __init__(self):
    super(MockSymbolResolver, self).__init__()
    self._map = collections.defaultdict(dict)

  def AddTestLibrarySymbols(self, lib_name, offsets_map):
    """Add a new test entry for a given library name.

    Args:
      lib_name: Library name (e.g. 'libfoo.so')
      offsets_map: A mapping from offsets to symbol info strings.
    """
    self._map[lib_name] = offsets_map

  def ReadTestFile(self, file_path, lib_name):
    """Read a single test symbol file, matching a given library.

    Args:
      file_path: Input file path.
      lib_name: Library name these symbols correspond to (e.g. 'libfoo.so')
    """
    with open(file_path) as f:
      for line in f.readlines():
        line = line.rstrip()
        m = MockSymbolResolver._RE_SYMBOL_DEFINITION.match(line)
        if m:
          offset = int(m.group('offset'))
          symbol = m.group('symbol')
          self._map[lib_name][offset] = symbol

  def ReadTestFilesInDir(self, dir_path, file_suffix):
    """Read all symbol test files in a given directory.

    Args:
      dir_path: Directory path.
      file_suffix: File suffix used to detect test symbol files.
    """
    for filename in os.listdir(dir_path):
      if filename.endswith(file_suffix):
        lib_name = filename[:-len(file_suffix)]
        self.ReadTestFile(os.path.join(dir_path, filename), lib_name)

  def FindSymbolInfo(self, device_path, device_offset):
    """Implement SymbolResolver.FindSymbolInfo."""
    lib_name = os.path.basename(device_path)
    offsets = self._map.get(lib_name)
    if not offsets:
      return None

    return offsets.get(device_offset)


class BacktraceTranslatorTest(unittest.TestCase):

  def testEmpty(self):
    android_abi = 'test-abi'
    apk_translator = MockApkTranslator()
    backtrace_translator = symbol_utils.BacktraceTranslator(android_abi,
                                                            apk_translator)
    self.assertTrue(backtrace_translator)

  def testFindLibraryOffsets(self):
    android_abi = 'test-abi'
    apk_translator = MockApkTranslator(_TEST_APK_LIBS)
    backtrace_translator = symbol_utils.BacktraceTranslator(android_abi,
                                                            apk_translator)
    input_backtrace = _EXPECTED_BACKTRACE.splitlines()
    expected_lib_offsets_map = _EXPECTED_BACKTRACE_OFFSETS_MAP
    offset_map = backtrace_translator.FindLibraryOffsets(input_backtrace)
    for lib_path, offsets in offset_map.iteritems():
      self.assertTrue(lib_path in expected_lib_offsets_map,
                      '%s is not in expected library-offsets map!' % lib_path)
      sorted_offsets = sorted(offsets)
      sorted_expected_offsets = sorted(expected_lib_offsets_map[lib_path])
      self.assertEqual(sorted_offsets, sorted_expected_offsets,
                       '%s has invalid offsets %s expected %s' % (
                          lib_path, sorted_offsets, sorted_expected_offsets))

  def testTranslateLine(self):
    android_abi = 'test-abi'
    apk_translator = MockApkTranslator(_TEST_APK_LIBS)
    backtrace_translator = symbol_utils.BacktraceTranslator(android_abi,
                                                            apk_translator)
    input_backtrace = _TEST_BACKTRACE.splitlines()
    expected_backtrace = _EXPECTED_BACKTRACE.splitlines()
    self.assertEqual(len(input_backtrace), len(expected_backtrace))
    for trace_line, expected_line in zip(input_backtrace, expected_backtrace):
      line = backtrace_translator.TranslateLine(trace_line,
                                                MockSymbolResolver())
      self.assertEqual(line, expected_line)


if __name__ == '__main__':
  unittest.main()
