//! Headless media adapters. These do not depend on the desktop repaint loop.
#[cfg(feature = "gstreamer-recording")]
pub mod program_recording;

#[cfg(feature = "gstreamer-capture")]
pub mod capture;
#[cfg(feature = "gstreamer-capture")]
pub mod capture_output;

#[cfg(feature = "gstreamer-capture")]
pub mod output_worker;
