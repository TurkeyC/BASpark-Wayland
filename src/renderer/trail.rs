//! 拖尾渲染 — 对应原 JS _updateTrail

use tiny_skia::{BlendMode, Color, GradientStop, LinearGradient, Paint, PathBuilder, PixmapMut, Point, Shader, SpreadMode, Stroke, Transform};

#[derive(Default, Clone)]
pub struct TrailPoint {
    pub x: f64,
    pub y: f64,
    pub life: f64,
}

pub struct TrailRenderer {
    pub points: Vec<TrailPoint>,
    pub max_trail: usize,
    pub always_trail: bool,
}

impl TrailRenderer {
    pub fn new(max_trail: usize) -> Self {
        Self {
            points: Vec::with_capacity(max_trail),
            max_trail,
            always_trail: false,
        }
    }

    pub fn render(
        &mut self,
        pixmap: &mut PixmapMut,
        head_pos: Option<(f64, f64)>,
        is_down: bool,
        frame_scale: f64,
        color: [u8; 3],
        _scale: f64,
    ) {
        let pts = &mut self.points;
        let base_decay = if self.always_trail {
            0.085 * frame_scale
        } else if is_down {
            0.085 * frame_scale
        } else {
            0.18 * frame_scale
        };

        let n = pts.len();
        let mut i: i32 = n as i32 - 1;
        while i >= 0 {
            let span = (n.max(1) - 1) as f64;
            let along = if n > 1 { i as f64 / span } else { 1.0 };
            let toward_cursor_bias = 1.25 - 0.55 * along;
            let mut step = base_decay * toward_cursor_bias;
            if step > 0.42 {
                step = 0.42;
            }
            let idx = i as usize;
            pts[idx].life -= step;
            if pts[idx].life <= 0.0 {
                pts.remove(idx);
            }
            i -= 1;
        }

        let mut all_pts: Vec<(f64, f64, f64)> =
            pts.iter().map(|p| (p.x, p.y, p.life)).collect();
        if let Some((hx, hy)) = head_pos {
            if !all_pts.is_empty() || is_down {
                all_pts.push((hx, hy, 1.0));
            }
        }

        if all_pts.len() < 2 {
            if let Some(&(px, py, life)) = all_pts.first() {
                let fade = (life * 0.85).max(0.0) as f32;
                let shader = Shader::SolidColor(Color::from_rgba8(
                    color[0],
                    color[1],
                    color[2],
                    (fade * 255.0) as u8,
                ));
                let paint = Paint {
                    shader,
                    blend_mode: BlendMode::Plus,
                    ..Default::default()
                };
                let pb = PathBuilder::from_circle(px as f32, py as f32, (2.5 + 2.0 * fade) as f32);
                if let Some(path) = pb {
                    pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
                }
            }
            return;
        }

        let last_idx = all_pts.len() - 1;
        for seg_i in 0..last_idx {
            let (x0, y0, _l0) = all_pts[seg_i];
            let (x1, y1, _l1) = all_pts[seg_i + 1];
            let astart = seg_i as f32 / last_idx as f32;
            let aend = (seg_i + 1) as f32 / last_idx as f32;
            draw_gradient_segment(pixmap, x0 as f32, y0 as f32, x1 as f32, y1 as f32, color, astart, aend);
        }
    }

    pub fn add_point(&mut self, x: f64, y: f64) {
        self.points.push(TrailPoint { x, y, life: 1.0 });
        if self.points.len() > self.max_trail {
            self.points.remove(0);
        }
    }

    pub fn has_work(&self) -> bool {
        !self.points.is_empty()
    }
}

fn draw_gradient_segment(
    pixmap: &mut PixmapMut,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: [u8; 3],
    alpha_start: f32,
    alpha_end: f32,
) {
    // 外层模糊模拟 pass
    if let Some(shader) = LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(color[0], color[1], color[2], (alpha_start * 0.3 * 255.0) as u8)),
            GradientStop::new(1.0, Color::from_rgba8(color[0], color[1], color[2], (alpha_end * 0.3 * 255.0) as u8)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        if let Some(path) = pb.finish() {
            let paint = Paint {
                shader,
                blend_mode: BlendMode::Plus,
                ..Default::default()
            };
            let stroke = Stroke {
                width: 10.0,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }

    // 内层实线 pass
    if let Some(shader) = LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(color[0], color[1], color[2], (alpha_start * 255.0) as u8)),
            GradientStop::new(1.0, Color::from_rgba8(color[0], color[1], color[2], (alpha_end * 255.0) as u8)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        if let Some(path) = pb.finish() {
            let paint = Paint {
                shader,
                blend_mode: BlendMode::Plus,
                ..Default::default()
            };
            let stroke = Stroke {
                width: 5.0,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}
