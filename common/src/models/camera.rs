//! Camera placements configured by modes and hooks, written as expressions the client
//! evaluates against its own screen.

use crate::types::{BoardCoord, ExprString, ExprVars, PieceTypeId};
use educe::Educe;
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Expression variable holding the zoom at which the whole board just fits on screen.
const MAX_FIT_ZOOM_VAR: &str = "max_fit_zoom";
/// Expression variable holding the centre of the square a piece died on, as an `(x, y)` tuple.
const DEATH_POSITION_VAR: &str = "death_position";
/// Expression variables holding the components of [`DEATH_POSITION_VAR`].
const DEATH_X_VAR: &str = "death_x";
const DEATH_Y_VAR: &str = "death_y";

/// Zoom at which the whole board just fits in the visible part of the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaxFitZoom(pub f64);

impl MaxFitZoom {
    /// Returns the variables every camera expression can read.
    fn vars(self) -> ExprVars {
        ExprVars::new().with(MAX_FIT_ZOOM_VAR, self.0)
    }
}

/// Point the camera centres on when a player joins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum StartingCameraPosition {
    /// The first piece of this type the player owns.
    Piece { piece_id: PieceTypeId },
    /// A fixed point, in squares from the board origin.
    Point([f64; 2]),
}

/// Where the camera looks, and how far it is zoomed, when a player joins a mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Educe)]
#[serde(default)]
#[educe(Default)]
pub struct StartingCameraConfig {
    #[educe(Default = StartingCameraPosition::Piece { piece_id: PieceTypeId::from("king") })]
    pub position: StartingCameraPosition,
    /// Zoom expression, which may read `max_fit_zoom`.
    #[educe(Default = ExprString::from("1"))]
    pub zoom: ExprString,
    /// Puts the camera back in the starting pose, fitted anew, whenever the screen is resized.
    #[educe(Default = false)]
    pub reset_on_resize: bool,
}

impl StartingCameraConfig {
    /// Returns the starting zoom on a screen fitting the board at `max_fit_zoom`.
    pub fn zoom(&self, max_fit_zoom: MaxFitZoom) -> f64 {
        self.zoom.evaluate(&max_fit_zoom.vars())
    }

    /// Checks that the zoom expression evaluates to a number.
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.zoom
            .check_number(&MaxFitZoom(1.0).vars())
            .map_err(|error| format!("starting_camera zoom: {error}"))
    }
}

/// Camera move a hook makes for the player it affects; an unset field leaves that part of the
/// camera where it is, so a move with neither field does nothing.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SetCameraConfig {
    /// Expression giving the point to centre on as an `(x, y)` tuple, in squares from the
    /// board origin.
    pub position: Option<ExprString>,
    /// Zoom expression.
    pub zoom: Option<ExprString>,
}

impl SetCameraConfig {
    /// Checks that the position evaluates to a pair of numbers and the zoom to a number.
    pub fn validate(&self) -> Result<(), String> {
        let sample = DeathVars {
            square: BoardCoord::new(0, 0),
            max_fit_zoom: MaxFitZoom(1.0),
        }
        .vars();
        if let Some(position) = &self.position {
            position
                .check_point(&sample)
                .map_err(|error| format!("set_camera position: {error}"))?;
        }
        if let Some(zoom) = &self.zoom {
            zoom.check_number(&sample)
                .map_err(|error| format!("set_camera zoom: {error}"))?;
        }
        Ok(())
    }
}

/// Values a camera move made at a piece's death can read.
struct DeathVars {
    square: BoardCoord,
    max_fit_zoom: MaxFitZoom,
}

impl DeathVars {
    /// Returns the variables bound to these values.
    fn vars(&self) -> ExprVars {
        let center = self.square.0.as_dvec2() + 0.5;
        self.max_fit_zoom
            .vars()
            .with(DEATH_POSITION_VAR, center)
            .with(DEATH_X_VAR, center.x)
            .with(DEATH_Y_VAR, center.y)
    }
}

/// Camera move a hook sent to a player when a piece died.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CameraCue {
    pub camera: SetCameraConfig,
    /// Square the piece died on.
    pub death_square: BoardCoord,
}

impl CameraCue {
    /// Returns where the cue moves the camera on a screen fitting the board at `max_fit_zoom`.
    pub fn target(&self, max_fit_zoom: MaxFitZoom) -> CameraTarget {
        let vars = DeathVars {
            square: self.death_square,
            max_fit_zoom,
        }
        .vars();
        CameraTarget {
            position: self
                .camera
                .position
                .as_ref()
                .and_then(|position| position.evaluate_point(&vars)),
            zoom: self.camera.zoom.as_ref().map(|zoom| zoom.evaluate(&vars)),
        }
    }
}

/// Evaluated camera move; unset parts leave the camera as it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraTarget {
    /// Point to centre on, in squares from the board origin.
    pub position: Option<DVec2>,
    pub zoom: Option<f64>,
}
