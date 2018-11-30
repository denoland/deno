// Copyright 2014 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// This file contains the default suppressions for ThreadSanitizer.
// You can also pass additional suppressions via TSAN_OPTIONS:
// TSAN_OPTIONS=suppressions=/path/to/suppressions. Please refer to
// http://dev.chromium.org/developers/testing/threadsanitizer-tsan-v2
// for more info.

#if defined(THREAD_SANITIZER)

// Please make sure the code below declares a single string variable
// kTSanDefaultSuppressions contains TSan suppressions delimited by newlines.
// See http://dev.chromium.org/developers/testing/threadsanitizer-tsan-v2
// for the instructions on writing suppressions.
char kTSanDefaultSuppressions[] =
    // False positives in libflashplayer.so, libgio.so and libglib.so.
    // Since we don't instrument them, we cannot reason about the
    // synchronization in them.
    "race:libflashplayer.so\n"
    "race:libgio*.so\n"
    "race:libglib*.so\n"

    // Intentional race in ToolsSanityTest.DataRace in base_unittests.
    "race:base/tools_sanity_unittest.cc\n"

    // Data race on WatchdogCounter [test-only].
    "race:base/threading/watchdog_unittest.cc\n"

    // Races in libevent, http://crbug.com/23244.
    "race:libevent/event.c\n"

    // http://crbug.com/84094.
    "race:sqlite3StatusSet\n"
    "race:pcache1EnforceMaxPage\n"
    "race:pcache1AllocPage\n"

    // http://crbug.com/120808
    "race:base/threading/watchdog.cc\n"

    // http://crbug.com/157586
    "race:third_party/libvpx/source/libvpx/vp8/decoder/threading.c\n"

    // http://crbug.com/158718
    "race:third_party/ffmpeg/libavcodec/pthread.c\n"
    "race:third_party/ffmpeg/libavcodec/pthread_frame.c\n"
    "race:third_party/ffmpeg/libavcodec/vp8.c\n"
    "race:third_party/ffmpeg/libavutil/mem.c\n"
    "race:*HashFrameForTesting\n"
    "race:third_party/ffmpeg/libavcodec/h264pred.c\n"
    "race:media::ReleaseData\n"

    // http://crbug.com/158922
    "race:third_party/libvpx/source/libvpx/vp8/encoder/*\n"
    "race:third_party/libvpx/source/libvpx/vp9/encoder/*\n"

    // http://crbug.com/239359
    "race:media::TestInputCallback::OnData\n"

    // http://crbug.com/244368
    "race:skia::BeginPlatformPaint\n"

    // http://crbug.com/244385
    "race:unixTempFileDir\n"

    // http://crbug.com/244755
    "race:v8::internal::Zone::NewExpand\n"
    "race:TooLateToEnableNow\n"
    "race:adjust_segment_bytes_allocated\n"

    // http://crbug.com/244774
    "race:webrtc::RTPReceiver::ProcessBitrate\n"
    "race:webrtc::RTPSender::ProcessBitrate\n"
    "race:webrtc::VideoCodingModuleImpl::Decode\n"
    "race:webrtc::RTPSender::SendOutgoingData\n"
    "race:webrtc::LibvpxVp8Encoder::GetEncodedPartitions\n"
    "race:webrtc::LibvpxVp8Encoder::Encode\n"
    "race:webrtc::ViEEncoder::DeliverFrame\n"
    "race:webrtc::vcm::VideoReceiver::Decode\n"
    "race:webrtc::VCMReceiver::FrameForDecoding\n"
    "race:*trace_event_unique_catstatic*\n"

    // http://crbug.com/244856
    "race:libpulsecommon*.so\n"

    // http://crbug.com/246968
    "race:webrtc::VideoCodingModuleImpl::RegisterPacketRequestCallback\n"

    // http://crbug.com/257396
    "race:base::trace_event::"
    "TraceEventTestFixture_TraceSamplingScope_Test::TestBody\n"

    // http://crbug.com/258479
    "race:SamplingStateScope\n"
    "race:g_trace_state\n"

    // http://crbug.com/258499
    "race:third_party/skia/include/core/SkRefCnt.h\n"

    // http://crbug.com/268924
    "race:base::g_power_monitor\n"
    "race:base::PowerMonitor::PowerMonitor\n"
    "race:base::PowerMonitor::AddObserver\n"
    "race:base::PowerMonitor::RemoveObserver\n"
    "race:base::PowerMonitor::IsOnBatteryPower\n"

    // http://crbug.com/258935
    "race:base::Thread::StopSoon\n"

    // http://crbug.com/272095
    "race:base::g_top_manager\n"

    // http://crbug.com/308590
    "race:CustomThreadWatcher::~CustomThreadWatcher\n"

    // http://crbug.com/310851
    "race:net::ProxyResolverV8Tracing::Job::~Job\n"

    // http://crbug.com/476529
    "deadlock:cc::VideoLayerImpl::WillDraw\n"

    // http://crbug.com/328826
    "race:gLCDOrder\n"
    "race:gLCDOrientation\n"

    // http://crbug.com/328868
    "race:PR_Lock\n"

    // http://crbug.com/333244
    "race:content::"
    "VideoCaptureImplTest::MockVideoCaptureImpl::~MockVideoCaptureImpl\n"

    // http://crbug.com/333871
    "race:v8::internal::Interface::NewValue()::value_interface\n"
    "race:v8::internal::IsMinusZero(double)::minus_zero\n"
    "race:v8::internal::FastCloneShallowObjectStub::"
    "InitializeInterfaceDescriptor\n"
    "race:v8::internal::KeyedLoadStubCompiler::registers\n"
    "race:v8::internal::KeyedStoreStubCompiler::registers()::registers\n"
    "race:v8::internal::KeyedLoadFastElementStub::"
    "InitializeInterfaceDescriptor\n"
    "race:v8::internal::KeyedStoreFastElementStub::"
    "InitializeInterfaceDescriptor\n"
    "race:v8::internal::LoadStubCompiler::registers\n"
    "race:v8::internal::StoreStubCompiler::registers\n"
    "race:v8::internal::HValue::LoopWeight\n"

    // http://crbug.com/334140
    "race:CommandLine::HasSwitch\n"
    "race:CommandLine::current_process_commandline_\n"
    "race:CommandLine::GetSwitchValueASCII\n"

    // http://crbug.com/338675
    "race:blink::s_platform\n"
    "race:content::"
    "RendererWebKitPlatformSupportImpl::~RendererWebKitPlatformSupportImpl\n"

    // http://crbug.com/347534
    "race:v8::internal::V8::TearDown\n"

    // http://crbug.com/347538
    "race:sctp_timer_start\n"

    // http://crbug.com/347553
    "race:blink::WebString::reset\n"

    // http://crbug.com/348511
    "race:webrtc::acm1::AudioCodingModuleImpl::PlayoutData10Ms\n"

    // http://crbug.com/348982
    "race:cricket::P2PTransportChannel::OnConnectionDestroyed\n"
    "race:cricket::P2PTransportChannel::AddConnection\n"

    // http://crbug.com/348984
    "race:sctp_express_handle_sack\n"
    "race:system_base_info\n"

    // https://code.google.com/p/v8/issues/detail?id=3143
    "race:v8::internal::FLAG_track_double_fields\n"

    // http://crbug.com/374135
    "race:media::AlsaWrapper::PcmWritei\n"

    // False positive in libc's tzset_internal, http://crbug.com/379738.
    "race:tzset_internal\n"

    // http://crbug.com/380554
    "deadlock:g_type_add_interface_static\n"

    // http:://crbug.com/386385
    "race:content::AppCacheStorageImpl::DatabaseTask::CallRunCompleted\n"

    // http://crbug.com/388730
    "race:g_next_user_script_id\n"

    // http://crbug.com/397022
    "deadlock:"
    "base::trace_event::TraceEventTestFixture_ThreadOnceBlocking_Test::"
    "TestBody\n"

    // http://crbug.com/415472
    "deadlock:base::trace_event::TraceLog::GetCategoryGroupEnabled\n"

    // http://crbug.com/490856
    "deadlock:content::TracingControllerImpl::SetEnabledOnFileThread\n"

    // https://code.google.com/p/skia/issues/detail?id=3294
    "race:SkBaseMutex::acquire\n"

    // https://crbug.com/430533
    "race:TileTaskGraphRunner::Run\n"

    // Lock inversion in third party code, won't fix.
    // https://crbug.com/455638
    "deadlock:dbus::Bus::ShutdownAndBlock\n"

    // https://crbug.com/459429
    "race:randomnessPid\n"

    // https://crbug.com/454655
    "race:content::BrowserTestBase::PostTaskToInProcessRendererAndWait\n"

    // https://crbug.com/569682
    "race:blink::ThreadState::visitStackRoots\n"

    // http://crbug.com/582274
    "race:usrsctp_close\n"

    // http://crbug.com/633145
    "race:third_party/libjpeg_turbo/simd/jsimd_x86_64.c\n"

    // http://crbug.com/587199
    "race:base::TimerTest_OneShotTimer_CustomTaskRunner_Test::TestBody\n"
    "race:base::TimerSequenceTest_OneShotTimerTaskOnPoolSequence_Test::"
    "TestBody\n"
    "race:base::TimerSequenceTest_"
    "OneShotTimerUsedAndTaskedOnDifferentSequences\n"

    // http://crbug.com/v8/6065
    "race:net::(anonymous namespace)::ProxyResolverV8TracingImpl::RequestImpl"
    "::~RequestImpl()\n"

    // http://crbug.com/691029
    "deadlock:libGLX.so*\n"

    // http://crbug.com/719633
    "race:crypto::EnsureNSSInit()\n"

    // http://crbug.com/695929
    "race:base::i18n::IsRTL\n"
    "race:base::i18n::SetICUDefaultLocale\n"

    // https://crbug.com/794920
    "race:base::debug::SetCrashKeyString\n"
    "race:crash_reporter::internal::CrashKeyStringImpl::Set\n"

    // http://crbug.com/795110
    "race:third_party/fontconfig/*\n"

    // http://crbug.com/797998
    "race:content::SandboxIPCHandler::HandleLocaltime\n"

    // End of suppressions.
    ;  // Please keep this semicolon.

#endif  // THREAD_SANITIZER
