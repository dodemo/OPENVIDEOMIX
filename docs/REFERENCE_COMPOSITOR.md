# Reference compositor: pixel-verified transitions

This module defines deterministic frame composition contracts, used as a **reference** for the production GPU compositor. The output is unpremultiplied RGBA8 and is intentionally independent of egui, GStreamer and wgpu.

Supported in the reference:
- CUT at the frame boundary, with stable Preview/Program source semantics defined separately.
- FADE via premultiplied-alpha interpolation (rounding to 8-bit RGBA at output).
- Horizontal WIPE revealing the target source from left to right.
- Four independent, ordered alpha overlays, with later layers above earlier layers.

Tests verify exact pixels at transition midpoint, alpha identities, z-order, dimensions and invalid progress. Color-management/linear-light compositing, video timestamps, zero-copy textures, hardware accelerated processing, scaling/cropping, transition interruptions, MERGE, Stingers and real media inputs remain **unimplemented** here.

**Important:** This does not make FADE/WIPE operational in the desktop application and does not encode or stream video. The CPU reference is not intended to process full-resolution real-time output; it is for correctness fixtures and future differential tests of wgpu/Metal/Direct3D/Vulkan output.

To validate:
```bash
cargo test -p openvideomix-core reference_compositor
cargo clippy -p openvideomix-core --all-targets -- -D warnings
```

Before declaring the GPU compositor complete:
1. Implement a common media-timebase and frame ownership contract.
2. Deliver frames from actual GStreamer sources to the compositor, not just egui thumbnails.
3. Compare GPU outputs with the reference pixel tests (with documented tolerance for color formats/rounding).
4. Add mixed color space/alpha, HDR, interlaced video and scaling rules as separate accepted specs.
5. Validate real 1080p50 output cadence and A/V sync on Windows/Linux/macOS.

Related: #3, #5, #6, #8.
