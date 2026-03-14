use yew::prelude::*;
use uuid::Uuid;
use common::*;
use common::logic::evaluate_expression;

pub struct CameraManager {
    pub camera: (f64, f64),
    pub target_camera: (f64, f64),
    pub zoom: f64,
    pub target_zoom: f64,
    pub mouse_pos: (f64, f64),
    pub velocity: (f64, f64),
    pub was_alive: bool,
    pub last_king_grid_pos: glam::IVec2,
    pub last_touch_dist: Option<f64>,
}

impl CameraManager {
    pub fn new() -> Self {
        Self {
            camera: (0.0, 0.0),
            target_camera: (0.0, 0.0),
            zoom: 1.0,
            target_zoom: 1.0,
            mouse_pos: (0.0, 0.0),
            velocity: (0.0, 0.0),
            was_alive: false,
            last_king_grid_pos: glam::IVec2::ZERO,
            last_touch_dist: None,
        }
    }
}

pub fn update_camera(
    manager: &mut CameraManager,
    state: &GameState,
    player_id: Option<Uuid>,
    canvas_ref: &NodeRef,
    is_dragging: bool,
    mode: Option<&common::models::GameModeConfig>,
    piece_count: usize,
) -> bool {
    let mut changed = false;
    let player_id_val = player_id.unwrap_or_else(Uuid::nil);
    let player = state.players.get(&player_id_val);
    let is_alive = player.is_some() && player_id_val != Uuid::nil();

    // 1. Zoom interpolation
    if (manager.target_zoom - manager.zoom).abs() > 0.000001 {
        let factor = 0.15;
        let old_z = manager.zoom;
        manager.zoom += (manager.target_zoom - manager.zoom) * factor;
        
        if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlElement>() {
            let rect = canvas.get_bounding_client_rect();
            let px = manager.mouse_pos.0 - rect.left();
            let py = manager.mouse_pos.1 - rect.top();
            let dx = px - rect.width() / 2.0;
            let dy = py - rect.height() / 2.0;
            let ratio = manager.zoom / old_z;
            manager.camera.0 = manager.camera.0 * ratio + dx * (ratio - 1.0);
            manager.camera.1 = manager.camera.1 * ratio + dy * (ratio - 1.0);
            changed = true;
        }
    }

    // 2. Velocity (inertia)
    if !is_dragging {
        if manager.velocity.0.abs() > 0.1 || manager.velocity.1.abs() > 0.1 {
            manager.camera.0 -= manager.velocity.0;
            manager.camera.1 -= manager.velocity.1;
            manager.velocity.0 *= 0.94;
            manager.velocity.1 *= 0.94;
            changed = true;
        } else {
            manager.velocity = (0.0, 0.0);
        }
    }

    // 3. King Following / Death Focusing
    if is_alive {
        if let Some(p) = player
            && let Some(king) = state.pieces.get(&p.king_id) {
            
            let tile_size = 40.0 * manager.zoom;
            let kpx = king.position.x as f64 * tile_size + tile_size / 2.0;
            let kpy = king.position.y as f64 * tile_size + tile_size / 2.0;

            if !manager.was_alive {
                // First time or respawn: snap camera to king
                manager.camera = (kpx, kpy);
                manager.last_king_grid_pos = king.position;
                manager.was_alive = true;
                changed = true;
            } else {
                // Just update the last position, don't move camera
                manager.last_king_grid_pos = king.position;
            }

            // 4. Global Clamping (Respect camera_pan_limit)
            if let Some(m) = mode {
                let mut vars = std::collections::HashMap::new();
                vars.insert("player_piece_count".to_string(), piece_count as f64);

                let fog_of_war_radius = evaluate_expression(&m.fog_of_war_radius, &vars);
                vars.insert("fog_of_war_radius".to_string(), fog_of_war_radius);

                let limit_radius_tiles = evaluate_expression(&m.camera_pan_limit, &vars);
                let limit_radius_px = limit_radius_tiles * tile_size;

                let dx = manager.camera.0 - kpx;
                let dy = manager.camera.1 - kpy;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist > limit_radius_px && dist > 0.1 {
                    if !is_dragging {
                        // Not dragging: Smoothly interpolate back to the allowed region
                        let target_dist = limit_radius_px;
                        let ratio = target_dist / dist;
                        let target_x = kpx + dx * ratio;
                        let target_y = kpy + dy * ratio;
                        
                        manager.camera.0 += (target_x - manager.camera.0) * 0.1;
                        manager.camera.1 += (target_y - manager.camera.1) * 0.1;
                        changed = true;
                    }
                }
            }
        }
    } else if manager.was_alive {
        // Just died, focus on last position
        let grid_pos = manager.last_king_grid_pos;
        let target_zoom = 1.3;
        let tile_size = 40.0 * target_zoom;
        manager.target_camera = (
            grid_pos.x as f64 * tile_size + tile_size / 2.0,
            grid_pos.y as f64 * tile_size + tile_size / 2.0
        );
        manager.target_zoom = target_zoom;
        manager.was_alive = false;
        changed = true;
    }

    if !is_alive && !is_dragging && ((manager.target_camera.0 - manager.camera.0).abs() > 0.1 || (manager.target_camera.1 - manager.camera.1).abs() > 0.1) {
        manager.camera.0 += (manager.target_camera.0 - manager.camera.0) * 0.1;
        manager.camera.1 += (manager.target_camera.1 - manager.camera.1) * 0.1;
        changed = true;
    }

    changed
}
