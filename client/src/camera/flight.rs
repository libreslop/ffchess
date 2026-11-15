//! Scripted camera flights: a pan and a zoom keyframed together, both easing out, the pan more
//! sharply than the zoom.

use crate::math::Vec2;

/// Where the camera looks and how far it is zoomed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    /// Point under the view center, in squares from the board origin.
    pub center: Vec2,
    pub zoom: f64,
}

/// How sharply a motion slows down. Every motion starts at full speed and slows down ever more
/// gently until it stops at the end of the flight, so the camera settles without a visible
/// cutoff.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sharpness(i32);

impl Sharpness {
    /// The pan is front-loaded, crossing most of the distance early.
    const PAN: Self = Self(5);
    /// The zoom slows down more gradually than the pan, so it is spread later.
    const ZOOM: Self = Self(2);

    /// Returns the progress of the motion, from 0 to 1, at the fraction `t` of the flight.
    /// Higher sharpness front-loads the motion more and gives it a longer, slower tail.
    fn progress(self, t: f64) -> f64 {
        1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(self.0)
    }
}

/// A camera move from one pose to another.
///
/// The pan and the zoom run over the whole flight at once, each keyframed with its own
/// sharpness.
#[derive(Clone, Debug)]
pub struct CameraFlight {
    from: CameraPose,
    to: CameraPose,
    duration_ms: f64,
    elapsed_ms: f64,
}

impl CameraFlight {
    /// Plans a flight from `from` to `to` lasting `duration_ms`.
    pub fn new(from: CameraPose, to: CameraPose, duration_ms: f64) -> Self {
        Self {
            from,
            to,
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
            center: self.from.center.lerp(self.to.center, Sharpness::PAN.progress(t)),
            // Zoom is eased by factor rather than linearly, so it feels equally fast at every
            // level.
            zoom: self.from.zoom * (self.to.zoom / self.from.zoom).powf(Sharpness::ZOOM.progress(t)),
        }
    }

    /// Returns true once the flight has reached its target.
    pub fn is_done(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }
}
