//! 火花三角形渲染 — 对应原 JS _updateSparks

use super::pool::Pool;
use tiny_skia::{BlendMode, Color, Paint, PathBuilder, PixmapMut, Shader, Transform};

#[derive(Default, Clone)]
pub struct Spark {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub rot: f64,
    pub rs: f64,
    pub s: f64,
    pub a: f64,
    pub f: f64,
    pub from_click: bool,
}

pub struct SparkRenderer {
    pub sparks: Vec<Spark>,
    pool: Pool<Spark>,
}

impl SparkRenderer {
    pub fn new() -> Self {
        Self {
            sparks: Vec::new(),
            pool: Pool::new(),
        }
    }

    pub fn create_spark(&mut self, x: f64, y: f64, from_click: bool, scale: f64) {
        let mut s = self.pool.acquire();
        let a = rand::random::<f64>() * std::f64::consts::PI * 2.0;
        let speed_adjust = scale / 1.5;
        let speed = if from_click {
            (4.8 + rand::random::<f64>() * 2.0) * speed_adjust
        } else {
            1.3 * speed_adjust
        };

        s.x = x;
        s.y = y;
        s.vx = a.cos() * speed;
        s.vy = a.sin() * speed;
        s.rot = rand::random::<f64>() * std::f64::consts::PI * 2.0;
        s.rs = (rand::random::<f64>() - 0.5) * 0.28;
        s.s = if from_click {
            (4.0 + rand::random::<f64>() * 3.0) * scale
        } else {
            9.0 * scale
        };
        s.a = if from_click { 1.0 } else { 0.7 };
        s.f = if from_click { 0.9 } else { 0.95 };
        s.from_click = from_click;

        self.sparks.push(s);
    }

    pub fn render(
        &mut self,
        pixmap: &mut PixmapMut,
        click_frame_scale: f64,
        trail_frame_scale: f64,
        opacity: f64,
    ) {
        let mut i: i32 = self.sparks.len() as i32 - 1;
        while i >= 0 {
            let idx = i as usize;
            let s = &mut self.sparks[idx];

            let fs = if s.from_click {
                click_frame_scale
            } else {
                trail_frame_scale
            };

            s.x += s.vx * fs;
            s.y += s.vy * fs;
            s.vx *= s.f.powf(fs);
            s.vy *= s.f.powf(fs);
            s.rot += s.rs * fs;
            s.a -= 0.032 * fs;

            if s.a <= 0.0 {
                let s = self.sparks.swap_remove(idx);
                self.pool.release(s);
                i -= 1;
                continue;
            }

            let alpha = (s.a * opacity).max(0.0).min(1.0);
            let paint = Paint {
                shader: Shader::SolidColor(Color::from_rgba8(
                    255,
                    255,
                    255,
                    (alpha * 255.0) as u8,
                )),
                blend_mode: BlendMode::Plus,
                ..Default::default()
            };

            let cos = (s.rot as f32).cos();
            let sin = (s.rot as f32).sin();
            let h = s.s as f32;

            let top = (-h, 0.0);
            let br = (h * 0.6, h * 0.6);
            let bl = (-h * 0.6, h * 0.6);

            let x = s.x as f32;
            let y = s.y as f32;

            let rt = |px: f32, py: f32| -> (f32, f32) {
                (x + px * cos - py * sin, y + px * sin + py * cos)
            };

            let (tx, ty) = rt(top.0, top.1);
            let (bx, by) = rt(br.0, br.1);
            let (lx, ly) = rt(bl.0, bl.1);

            let mut pb = PathBuilder::new();
            pb.move_to(tx, ty);
            pb.line_to(bx, by);
            pb.line_to(lx, ly);
            pb.close();

            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            i -= 1;
        }
    }

    pub fn has_work(&self) -> bool {
        !self.sparks.is_empty()
    }
}
