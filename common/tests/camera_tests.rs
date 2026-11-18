//! Tests for camera expressions configured by modes and hooks.

use common::models::{
    CameraCue, GameModeConfig, MaxFitZoom, SetCameraConfig, StartingCameraConfig,
    StartingCameraPosition,
};
use common::types::{BoardCoord, ExprString, ExprVars};
use glam::DVec2;

/// Returns a cue for a piece that died on `(x, y)`.
fn cue(position: Option<&str>, zoom: Option<&str>, x: i32, y: i32) -> CameraCue {
    CameraCue {
        camera: SetCameraConfig {
            position: position.map(ExprString::from),
            zoom: zoom.map(ExprString::from),
        },
        death_square: BoardCoord::new(x, y),
    }
}

#[test]
/// Verifies point expressions evaluate `(x, y)` tuples, including arithmetic on components.
fn evaluate_point_reads_tuples() {
    let vars = ExprVars::new().with("a", 2.0);
    assert_eq!(
        ExprString::from("(a * 2, a - 3)").evaluate_point(&vars),
        Some(DVec2::new(4.0, -1.0))
    );
    assert_eq!(ExprString::from("a").evaluate_point(&vars), None);
    assert_eq!(ExprString::from("(1, 2, 3)").evaluate_point(&vars), None);
}

#[test]
/// Verifies a cue centres on the middle of the death square and reads the fit zoom.
fn cue_targets_death_square() {
    let target =
        cue(Some("death_position"), Some("0.5 * max_fit_zoom"), 3, -2).target(MaxFitZoom(4.0));
    assert_eq!(target.position, Some(DVec2::new(3.5, -1.5)));
    assert_eq!(target.zoom, Some(2.0));

    let offset = cue(Some("(death_x + 1, death_y)"), None, 0, 0).target(MaxFitZoom(1.0));
    assert_eq!(offset.position, Some(DVec2::new(1.5, 0.5)));
    assert_eq!(offset.zoom, None);
}

#[test]
/// Verifies a cue without fields leaves the camera where it is.
fn empty_cue_moves_nothing() {
    let target = cue(None, None, 1, 1).target(MaxFitZoom(1.0));
    assert_eq!(target.position, None);
    assert_eq!(target.zoom, None);
}

#[test]
/// Verifies the starting zoom reads the fit zoom.
fn starting_zoom_reads_fit_zoom() {
    let camera = StartingCameraConfig {
        position: StartingCameraPosition::Point([0.0, 0.0]),
        zoom: ExprString::from("0.8 * max_fit_zoom"),
        reset_on_resize: false,
    };
    assert_eq!(camera.zoom(MaxFitZoom(2.5)), 2.0);
}

#[test]
/// Verifies the starting camera accepts a fixed point or a piece.
fn starting_camera_parses_both_positions() {
    let point: StartingCameraConfig = serde_json::from_str(
        r#"{ "position": [1, -2], "zoom": "max_fit_zoom", "reset_on_resize": true }"#,
    )
    .unwrap();
    assert_eq!(point.position, StartingCameraPosition::Point([1.0, -2.0]));
    assert!(point.reset_on_resize);
    let piece: StartingCameraConfig =
        serde_json::from_str(r#"{ "position": { "piece_id": "king" } }"#).unwrap();
    assert!(matches!(
        piece.position,
        StartingCameraPosition::Piece { .. }
    ));
    assert_eq!(piece.zoom, ExprString::from("1"));
    assert!(!piece.reset_on_resize);
}

#[test]
/// Verifies malformed camera expressions are rejected when a mode validates.
fn invalid_camera_expressions_fail_validation() {
    let bad_zoom = GameModeConfig {
        starting_camera: StartingCameraConfig {
            zoom: ExprString::from("max_fit + 1"),
            ..StartingCameraConfig::default()
        },
        ..GameModeConfig::default()
    };
    assert!(bad_zoom.validate().is_err());

    assert!(
        SetCameraConfig {
            position: Some(ExprString::from("death_x")),
            zoom: None,
        }
        .validate()
        .is_err()
    );
    assert!(
        SetCameraConfig {
            position: Some(ExprString::from("death_position")),
            zoom: Some(ExprString::from("max_fit_zoom")),
        }
        .validate()
        .is_ok()
    );
    assert!(GameModeConfig::default().validate().is_ok());
}
