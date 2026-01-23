//! Canvas 2D rendering system for the game.

#![allow(deprecated)] // web-sys Canvas API deprecation warnings

use marble_core::map::{Obstacle, Wall};
use marble_core::{Color, GameState, Marble, RouletteConfig};
use std::f64::consts::PI;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// Canvas renderer for drawing the game state.
pub struct CanvasRenderer {
    context: CanvasRenderingContext2d,
    width: f64,
    height: f64,
}

impl CanvasRenderer {
    /// Creates a new canvas renderer from an HTML canvas element.
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let context = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("Failed to get 2d context"))?
            .dyn_into::<CanvasRenderingContext2d>()?;

        let width = f64::from(canvas.width());
        let height = f64::from(canvas.height());

        Ok(Self {
            context,
            width,
            height,
        })
    }

    /// Clears the canvas with a background color.
    pub fn clear(&self) {
        self.context.set_fill_style(&JsValue::from_str("#1a1a2e"));
        self.context.fill_rect(0.0, 0.0, self.width, self.height);
    }

    /// Renders the complete game state.
    pub fn render(&self, game_state: &GameState) {
        self.clear();

        // Draw map elements
        if let Some(config) = &game_state.map_config {
            self.draw_roulette(config);
        }

        // Draw marbles
        for marble in game_state.marble_manager.marbles() {
            if let Some((x, y)) = game_state
                .marble_manager
                .get_marble_position(&game_state.physics_world, marble.id)
            {
                self.draw_marble(marble, x, y);
            }
        }

        // Draw countdown if applicable
        if let marble_core::GamePhase::Countdown { remaining_frames } = game_state.current_phase() {
            self.draw_countdown(*remaining_frames);
        }
    }

    /// Draws the roulette map (walls, obstacles, holes).
    pub fn draw_roulette(&self, config: &RouletteConfig) {
        // Draw holes first (background)
        for hole in &config.holes {
            self.draw_hole(hole.center[0], hole.center[1], hole.radius);
        }

        // Draw walls
        for wall in &config.walls {
            match wall {
                Wall::Line(line) => {
                    self.draw_wall_line(
                        line.start[0],
                        line.start[1],
                        line.end[0],
                        line.end[1],
                    );
                }
            }
        }

        // Draw obstacles
        for obstacle in &config.obstacles {
            match obstacle {
                Obstacle::Circle(circle) => {
                    self.draw_obstacle_circle(
                        circle.center[0],
                        circle.center[1],
                        circle.radius,
                    );
                }
                Obstacle::Rect(rect) => {
                    self.draw_obstacle_rect(
                        rect.center[0],
                        rect.center[1],
                        rect.size[0],
                        rect.size[1],
                        rect.rotation,
                    );
                }
            }
        }

        // Draw spawn area (debug visualization)
        self.draw_spawn_area(&config.spawn_area);
    }

    /// Draws a hole (elimination zone).
    fn draw_hole(&self, x: f32, y: f32, radius: f32) {
        let ctx = &self.context;

        // Draw hole shadow/glow
        ctx.save();
        ctx.begin_path();
        let _ = ctx.arc(
            f64::from(x),
            f64::from(y),
            f64::from(radius * 1.2),
            0.0,
            2.0 * PI,
        );
        ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.5)"));
        ctx.fill();

        // Draw hole
        ctx.begin_path();
        let _ = ctx.arc(
            f64::from(x),
            f64::from(y),
            f64::from(radius),
            0.0,
            2.0 * PI,
        );
        ctx.set_fill_style(&JsValue::from_str("#0d0d1a"));
        ctx.fill();

        // Draw hole border
        ctx.set_stroke_style(&JsValue::from_str("#ff4444"));
        ctx.set_line_width(3.0);
        ctx.stroke();
        ctx.restore();
    }

    /// Draws a wall line segment.
    fn draw_wall_line(&self, x1: f32, y1: f32, x2: f32, y2: f32) {
        let ctx = &self.context;

        ctx.save();
        ctx.begin_path();
        ctx.move_to(f64::from(x1), f64::from(y1));
        ctx.line_to(f64::from(x2), f64::from(y2));
        ctx.set_stroke_style(&JsValue::from_str("#4a4a6a"));
        ctx.set_line_width(4.0);
        ctx.set_line_cap("round");
        ctx.stroke();
        ctx.restore();
    }

    /// Draws a circular obstacle.
    fn draw_obstacle_circle(&self, x: f32, y: f32, radius: f32) {
        let ctx = &self.context;

        ctx.save();
        ctx.begin_path();
        let _ = ctx.arc(
            f64::from(x),
            f64::from(y),
            f64::from(radius),
            0.0,
            2.0 * PI,
        );
        ctx.set_fill_style(&JsValue::from_str("#3a3a5a"));
        ctx.fill();
        ctx.set_stroke_style(&JsValue::from_str("#5a5a8a"));
        ctx.set_line_width(2.0);
        ctx.stroke();
        ctx.restore();
    }

    /// Draws a rectangular obstacle.
    fn draw_obstacle_rect(&self, x: f32, y: f32, width: f32, height: f32, rotation: f32) {
        let ctx = &self.context;

        ctx.save();
        ctx.translate(f64::from(x), f64::from(y)).unwrap_or(());
        ctx.rotate(f64::from(rotation.to_radians())).unwrap_or(());

        let half_w = f64::from(width) / 2.0;
        let half_h = f64::from(height) / 2.0;

        ctx.begin_path();
        ctx.rect(-half_w, -half_h, f64::from(width), f64::from(height));
        ctx.set_fill_style(&JsValue::from_str("#3a3a5a"));
        ctx.fill();
        ctx.set_stroke_style(&JsValue::from_str("#5a5a8a"));
        ctx.set_line_width(2.0);
        ctx.stroke();
        ctx.restore();
    }

    /// Draws the spawn area (debug visualization).
    fn draw_spawn_area(&self, spawn_area: &marble_core::map::SpawnArea) {
        let ctx = &self.context;

        ctx.save();
        ctx.set_stroke_style(&JsValue::from_str("rgba(100, 200, 100, 0.3)"));
        ctx.set_line_width(1.0);
        ctx.set_line_dash(&js_sys::Array::of2(&JsValue::from(5), &JsValue::from(5)))
            .unwrap_or(());
        ctx.stroke_rect(
            f64::from(spawn_area.x[0]),
            f64::from(spawn_area.y[0]),
            f64::from(spawn_area.x[1] - spawn_area.x[0]),
            f64::from(spawn_area.y[1] - spawn_area.y[0]),
        );
        ctx.restore();
    }

    /// Draws a marble with its color and position.
    pub fn draw_marble(&self, marble: &Marble, x: f32, y: f32) {
        let ctx = &self.context;

        if marble.eliminated {
            return;
        }

        let color = &marble.color;
        let radius = f64::from(marble.radius);

        ctx.save();

        // Draw shadow
        ctx.begin_path();
        let _ = ctx.arc(
            f64::from(x) + 2.0,
            f64::from(y) + 2.0,
            radius,
            0.0,
            2.0 * PI,
        );
        ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.3)"));
        ctx.fill();

        // Draw marble body
        ctx.begin_path();
        let _ = ctx.arc(f64::from(x), f64::from(y), radius, 0.0, 2.0 * PI);

        let fill_color = format!("rgb({}, {}, {})", color.r, color.g, color.b);
        ctx.set_fill_style(&JsValue::from_str(&fill_color));
        ctx.fill();

        // Draw highlight
        ctx.begin_path();
        let _ = ctx.arc(
            f64::from(x) - radius * 0.3,
            f64::from(y) - radius * 0.3,
            radius * 0.3,
            0.0,
            2.0 * PI,
        );
        ctx.set_fill_style(&JsValue::from_str("rgba(255, 255, 255, 0.4)"));
        ctx.fill();

        // Draw border
        ctx.begin_path();
        let _ = ctx.arc(f64::from(x), f64::from(y), radius, 0.0, 2.0 * PI);
        let stroke_color = format!(
            "rgb({}, {}, {})",
            color.r.saturating_sub(40),
            color.g.saturating_sub(40),
            color.b.saturating_sub(40)
        );
        ctx.set_stroke_style(&JsValue::from_str(&stroke_color));
        ctx.set_line_width(2.0);
        ctx.stroke();

        ctx.restore();
    }

    /// Draws countdown text.
    fn draw_countdown(&self, remaining_frames: u32) {
        let ctx = &self.context;
        let seconds = (remaining_frames as f64 / 60.0).ceil() as u32;

        ctx.save();
        ctx.set_font("bold 72px sans-serif");
        ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");

        // Draw shadow
        ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.5)"));
        let _ = ctx.fill_text(
            &seconds.to_string(),
            self.width / 2.0 + 3.0,
            self.height / 2.0 + 3.0,
        );

        // Draw text
        ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        let _ = ctx.fill_text(&seconds.to_string(), self.width / 2.0, self.height / 2.0);

        ctx.restore();
    }

    /// Draws winner announcement.
    fn draw_winner(&self, winner: Option<u32>, game_state: &GameState) {
        let ctx = &self.context;

        let text = match winner {
            Some(id) => {
                let player_name = game_state
                    .get_player(id)
                    .map(|p| p.name.as_str())
                    .unwrap_or("Unknown");
                format!("{player_name} Wins!")
            }
            None => "No Winner!".to_string(),
        };

        ctx.save();

        // Draw overlay
        ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.6)"));
        ctx.fill_rect(0.0, 0.0, self.width, self.height);

        ctx.set_font("bold 48px sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");

        // Draw shadow
        ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.5)"));
        let _ = ctx.fill_text(&text, self.width / 2.0 + 3.0, self.height / 2.0 + 3.0);

        // Draw text
        ctx.set_fill_style(&JsValue::from_str("#ffd700"));
        let _ = ctx.fill_text(&text, self.width / 2.0, self.height / 2.0);

        ctx.restore();
    }
}

/// Converts a Color to a CSS color string.
pub fn color_to_css(color: &Color) -> String {
    format!("rgba({}, {}, {}, {})", color.r, color.g, color.b, color.a)
}
