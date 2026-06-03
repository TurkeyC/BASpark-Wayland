//! 粒子渲染引擎主控 — 对应原 JS MouseSpark 类

pub mod dirty_rect;
pub mod pool;
pub mod spark;
pub mod trail;
pub mod wave;

use crate::config::{rings_end_color, Config};
use self::dirty_rect::{point_rect, segment_rect, DirtyRectTracker, Rect};
use self::spark::Spark;
use self::trail::TrailRenderer;
use self::wave::WaveRenderer;
use tiny_skia::{Pixmap, PixmapPaint, Transform};
use std::time::Instant;

pub struct ParticleEngine {
    buffer: Pixmap,
    main: Pixmap,
    trail: TrailRenderer,
    waves: WaveRenderer,
    sparks: spark::SparkRenderer,
    dirty_tracker: DirtyRectTracker,
    is_down: bool,
    last_pos: Option<(f64, f64)>,
    last_frame_time: Instant,
    base_frame_secs: f64,
    max_delta_secs: f64,
    pub css_width: f64,
    pub css_height: f64,
    force_full_redraw: bool,
}

impl ParticleEngine {
    pub fn new(config: &Config) -> Self {
        let color = config.parse_color();
        let rings_end = rings_end_color(color[0], color[1], color[2]);

        let mut waves = WaveRenderer::new();
        waves.set_color(color, rings_end);

        Self {
            buffer: Pixmap::new(1, 1).unwrap(),
            main: Pixmap::new(1, 1).unwrap(),
            trail: TrailRenderer::new(16), // max 16 trail points
            waves,
            sparks: spark::SparkRenderer::new(),
            dirty_tracker: DirtyRectTracker::new(),
            is_down: false,
            last_pos: None,
            last_frame_time: Instant::now(),
            base_frame_secs: 1.0 / 60.0,
            max_delta_secs: 0.1,
            css_width: 1.0,
            css_height: 1.0,
            force_full_redraw: true,
        }
    }

    pub fn resize(&mut self, width: f64, height: f64) {
        let w = (width.max(1.0) as u32).max(1);
        let h = (height.max(1.0) as u32).max(1);
        self.buffer = Pixmap::new(w, h).unwrap();
        self.main = Pixmap::new(w, h).unwrap();
        self.css_width = width;
        self.css_height = height;
        self.force_full_redraw = true;
    }

    pub fn on_mouse_down(&mut self, x: f64, y: f64, config: &Config) {
        if !config.is_effect_enabled {
            return;
        }

        self.is_down = true;
        self.last_pos = Some((x, y));
        let scale = config.effect_scale;

        self.waves.create_wave(x, y);
        for _ in 0..4 {
            self.sparks.create_spark(x, y, true, scale);
        }
    }

    pub fn on_mouse_move(&mut self, x: f64, y: f64, config: &Config) {
        let prev = self.last_pos;
        self.last_pos = Some((x, y));

        if !config.is_effect_enabled {
            return;
        }
        if !self.is_down && !config.enable_always_trail {
            return;
        }
        if let Some((px, py)) = prev {
            if ((x - px).powi(2) + (y - py).powi(2)).sqrt() > 2.0 {
                self.trail.add_point(x, y);

                if rand::random::<f64>() < 0.3 {
                    let angle = rand::random::<f64>() * std::f64::consts::PI * 2.0;
                    let scale = config.effect_scale;
                    let speed_adjust = scale / 1.5;

                    let mut spark = Spark::default();
                    spark.x = x + angle.cos() * 10.0 * scale;
                    spark.y = y + angle.sin() * 10.0 * scale;
                    spark.vx = angle.cos() * 1.3 * speed_adjust;
                    spark.vy = angle.sin() * 1.3 * speed_adjust;
                    spark.rot = rand::random::<f64>() * std::f64::consts::PI * 2.0;
                    spark.rs = 0.16;
                    spark.s = 9.0 * scale;
                    spark.a = 0.7;
                    spark.f = 0.95;
                    spark.from_click = false;
                    self.sparks.sparks.push(spark);
                }
            }
        }
    }

    pub fn on_mouse_up(&mut self) {
        self.is_down = false;
    }

    /// 返回像素数据 (premultiplied BGRA)
    pub fn render_frame(&mut self, config: &Config) -> Option<&[u8]> {
        let (trail_speed, click_speed) = config.get_animation_speeds();

        let has_work = self.waves.has_work()
            || self.sparks.has_work()
            || self.trail.has_work()
            || self.is_down;

        if !has_work {
            self.last_frame_time = Instant::now();
            if !self.dirty_tracker.get_render_rects(&[]).is_empty() {
                let w = (self.css_width.max(1.0) as u32).max(1);
                let h = (self.css_height.max(1.0) as u32).max(1);
                self.buffer = Pixmap::new(w, h).unwrap();
                self.dirty_tracker.set_dirty_rects(vec![]);
            }
            return None;
        }

        let now = Instant::now();
        let delta_secs = now
            .duration_since(self.last_frame_time)
            .as_secs_f64()
            .min(self.max_delta_secs);
        self.last_frame_time = now;

        let frame_scale = delta_secs / self.base_frame_secs;
        let trail_fs = frame_scale * trail_speed;
        let click_fs = frame_scale * click_speed;

        let color = config.parse_color();
        let scale = config.effect_scale;

        // 清除 buffer
        self.buffer.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0));

        // 渲染各子系统
        {
            let mut pm = self.buffer.as_mut();
            self.trail.render(&mut pm, self.last_pos, self.is_down, trail_fs, color, scale);
            self.waves.render(&mut pm, click_fs, color, config.effect_opacity);
            self.sparks.render(&mut pm, click_fs, trail_fs, config.effect_opacity);
        }

        // 将 buffer 合成到 main
        self.main.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0));
        self.main.as_mut().draw_pixmap(
            0,
            0,
            self.buffer.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );

        let effect_rects = self.get_effect_rects(scale);
        let _render_rects = self.dirty_tracker.get_render_rects(&effect_rects);
        self.dirty_tracker.set_dirty_rects(effect_rects);
        self.force_full_redraw = false;

        Some(self.main.data())
    }

    fn get_effect_rects(&self, scale: f64) -> Vec<Rect> {
        let mut rects = Vec::new();
        let trail_pad = 18.0 * scale + 12.0;

        let trail_pts: Vec<(f64, f64)> = self
            .trail
            .points
            .iter()
            .map(|p| (p.x, p.y))
            .chain(self.last_pos.iter().copied())
            .collect();

        if trail_pts.len() == 1 {
            rects.push(point_rect(trail_pts[0].0, trail_pts[0].1, trail_pad));
        } else {
            for i in 0..trail_pts.len().saturating_sub(1) {
                rects.push(segment_rect(trail_pts[i], trail_pts[i + 1], trail_pad));
            }
        }

        let wave_pad = 34.0 * scale + 3.3 + 16.0;
        for w in &self.waves.waves {
            rects.push(point_rect(w.x, w.y, 26.0 * scale + wave_pad));
        }

        for s in &self.sparks.sparks {
            let pad = s.s.max(9.0 * scale) * 2.0 + 12.0;
            rects.push(point_rect(s.x, s.y, pad));
        }

        rects
    }

    pub fn has_work(&self) -> bool {
        self.waves.has_work()
            || self.sparks.has_work()
            || self.trail.has_work()
            || self.is_down
    }

    pub fn pixels(&self) -> &[u8] {
        self.main.data()
    }
}
