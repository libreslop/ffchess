//! Scripted camera flights: a pan and a zoom keyframed together, one weighted early and the
//! other late.

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

/// How a motion's progress is spread over the flight. Every curve starts and settles gently.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Easing {
    /// Moves fastest a third of the way in, so most of the motion happens early.
    Early,
    /// Moves fastest two thirds of the way in, so most of the motion happens late.
    Late,
    /// Moves fastest halfway, for a flight with a single motion.
    Even,
}

impl Easing {
    /// Returns the progress of the motion, from 0 to 1, at the fraction `t` of the flight.
    fn progress(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            // Integral of the speed curve t(1 - t)², scaled to end at 1.
            Self::Early => t * t * (6.0 - 8.0 * t + 3.0 * t * t),
            Self::Late => 1.0 - Self::Early.progress(1.0 - t),
            Self::Even => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// A camera move from one pose to another.
///
/// The pan and the zoom run over the whole flight at once. Zooming in pans mostly early, while
/// the view is still wide, and zooms mostly late; zooming out widens the view mostly early and
/// pans mostly late.
#[derive(Clone, Debug)]
pub struct CameraFlight {
    from: CameraPose,
    to: CameraPose,
    pan: Easing,
    zoom: Easing,
    duration_ms: f64,
    elapsed_ms: f64,
}

impl CameraFlight {
    /// Plans a flight from `from` to `to` lasting `duration_ms`. A motion that would barely
    /// move leaves the other one to ease evenly on its own.
    pub fn new(from: CameraPose, to: CameraPose, duration_ms: f64) -> Self {
        let pans = (to.center - from.center).length() > MIN_PAN_SQUARES;
        let zooms = (to.zoom / from.zoom).ln().abs() > MIN_ZOOM_LOG;
        let (pan, zoom) = match (pans && zooms, to.zoom >= from.zoom) {
            (false, _) => (Easing::Even, Easing::Even),
            (true, true) => (Easing::Early, Easing::Late),
            (true, false) => (Easing::Late, Easing::Early),
        };
        Self {
            from,
            to,
            pan,
            zoom,
            duration_ms: duration_ms.max(0.0),
            elapsed_ms: 0.0,
        }
    }

    /// Moves the flight on by `elapsed_ms` and returns the pose the camera has reached.
    pub fn advance(&mut self, elapsed_ms: f64) -> CameraPose {
        self.elapsed_ms += elapsed_ms;
        if self.is_done() {
            return self.to;
        }
        let t = self.elapsed_ms / self.duration_ms;
        CameraPose {
            center: self.from.center.lerp(self.to.center, self.pan.progress(t)),
            // Zoom is eased by factor rather than linearly, so it feels equally fast at every
            // level.
            zoom: self.from.zoom * (self.to.zoom / self.from.zoom).powf(self.zoom.progress(t)),
        }
    }

    /// Returns true once the flight has reached its target.
    pub fn is_done(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }
}
