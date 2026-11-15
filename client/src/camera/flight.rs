//! Scripted camera flights: a pan and a zoom eased one after the other, slightly overlapping.

use crate::math::Vec2;

/// Pans shorter than this many squares are skipped.
const MIN_PAN_SQUARES: f64 = 0.01;
/// Zooms changing the zoom by a smaller factor than this, in natural log, are skipped.
const MIN_ZOOM_LOG: f64 = 0.001;

/// Where the camera looks and how far it is zoomed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    /// Point under the view center, in squares from the board origin.
    pub center: Vec2,
    pub zoom: f64,
}

/// Durations of the motions of a flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightTiming {
    pub pan_ms: f64,
    pub zoom_ms: f64,
    /// How long the second motion starts before the first one ends.
    pub overlap_ms: f64,
}

/// The stretch of a flight one motion runs over, in milliseconds since the flight began.
#[derive(Clone, Copy, Debug)]
struct Phase {
    start_ms: f64,
    duration_ms: f64,
}

impl Phase {
    /// Returns the eased progress of the motion at `elapsed_ms`: 0 before it starts and 1 once
    /// it has ended.
    fn progress(&self, elapsed_ms: f64) -> f64 {
        if self.duration_ms <= 0.0 {
            return 1.0;
        }
        let t = ((elapsed_ms - self.start_ms) / self.duration_ms).clamp(0.0, 1.0);
        // Cubic ease in and out, so each motion starts and settles gently.
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (2.0 - 2.0 * t).powi(3) / 2.0
        }
    }

    /// Returns when the motion ends, in milliseconds since the flight began.
    fn end_ms(&self) -> f64 {
        self.start_ms + self.duration_ms
    }
}

/// A camera move from one pose to another.
///
/// Zooming in pans first, while the view is still wide, then zooms in on the target; zooming
/// out widens the view first, then pans. The second motion starts slightly before the first
/// ends, so the camera never stops in between.
#[derive(Clone, Debug)]
pub struct CameraFlight {
    from: CameraPose,
    to: CameraPose,
    pan: Phase,
    zoom: Phase,
    elapsed_ms: f64,
}

impl CameraFlight {
    /// Plans a flight from `from` to `to`, skipping a motion that would barely move.
    pub fn new(from: CameraPose, to: CameraPose, timing: FlightTiming) -> Self {
        let pans = (to.center - from.center).length() > MIN_PAN_SQUARES;
        let zooms = (to.zoom / from.zoom).ln().abs() > MIN_ZOOM_LOG;
        let pan_ms = if pans { timing.pan_ms.max(0.0) } else { 0.0 };
        let zoom_ms = if zooms { timing.zoom_ms.max(0.0) } else { 0.0 };
        let overlap_ms = if pans && zooms {
            timing.overlap_ms.clamp(0.0, pan_ms.min(zoom_ms))
        } else {
            0.0
        };
        let (pan, zoom) = if to.zoom >= from.zoom {
            let pan = Phase {
                start_ms: 0.0,
                duration_ms: pan_ms,
            };
            let zoom = Phase {
                start_ms: pan.end_ms() - overlap_ms,
                duration_ms: zoom_ms,
            };
            (pan, zoom)
        } else {
            let zoom = Phase {
                start_ms: 0.0,
                duration_ms: zoom_ms,
            };
            let pan = Phase {
                start_ms: zoom.end_ms() - overlap_ms,
                duration_ms: pan_ms,
            };
            (pan, zoom)
        };
        Self {
            from,
            to,
            pan,
            zoom,
            elapsed_ms: 0.0,
        }
    }

    /// Moves the flight on by `elapsed_ms` and returns the pose the camera has reached.
    pub fn advance(&mut self, elapsed_ms: f64) -> CameraPose {
        self.elapsed_ms += elapsed_ms;
        if self.is_done() {
            return self.to;
        }
        let pan = self.pan.progress(self.elapsed_ms);
        let zoom = self.zoom.progress(self.elapsed_ms);
        CameraPose {
            center: self.from.center.lerp(self.to.center, pan),
            // Zoom is eased by factor rather than linearly, so it feels equally fast at every
            // level.
            zoom: self.from.zoom * (self.to.zoom / self.from.zoom).powf(zoom),
        }
    }

    /// Returns true once both motions have ended.
    pub fn is_done(&self) -> bool {
        self.elapsed_ms >= self.pan.end_ms().max(self.zoom.end_ms())
    }
}
