# GStreamer cross-platform camera discovery (experimental)

The optional `gstreamer-capture` feature exposes a background snapshot of video source devices reported by GStreamer's `GstDeviceMonitor` under the `Video/Source` class. In Settings → Capture Devices, use **Explorar cámaras GStreamer**. The scan runs in a worker thread; the UI polls the result without blocking for device enumeration. The list displays names, device classes and capabilities reported by the active providers.

This is **discovery only**. There is no selection by stable device ID and no binding of a specific listed camera to a capture pipeline in this change. The existing webcam button still opens the default device, and multiple such sessions can collide if only one physical webcam is available.

GStreamer providers and permissions vary among Windows, Fedora/other Linux distributions, Intel Macs and Apple Silicon. An empty list may mean no installed device provider, missing permissions, or genuinely no attached cameras. On GStreamer 1.28 and newer, device-monitor startup can be asynchronous; the current unstarted `devices()` query performs a snapshot probe and is not a hotplug subscription.

## Acceptance criteria for the next stage

1. Give each device a stable persisted identity when one is available; distinguish duplicate display names, and fail safely if an identity becomes unavailable.
2. Use `GstDevice::create_element()` for physical selection instead of embedding untrusted paths or labels into pipeline descriptions.
3. Move opening/reopening to dedicated workers; report missing permissions, unplug/replug and selected-format failures in the UI.
4. Negotiate actual `CameraMode` capabilities (including 1920×1080 at 50/1 only when the device advertises it) and measure dropped frames and frame timestamps.
5. Validate OBSBOT Tiny 2 and a second UVC device physically on Windows, Linux and macOS, testing USB mode, permissions and data rate.

See the [GStreamer DeviceMonitor documentation](https://gstreamer.freedesktop.org/documentation/gstreamer/gstdevicemonitor.html) for the documented discovery pattern.

This discovery functionality is part of a draft PR and must not be described as an implemented capability of `main` until that PR is merged.
