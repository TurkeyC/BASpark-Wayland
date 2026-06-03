//! 点击波纹渲染 — 对应原 JS _updateWaves

use super::pool::Pool;
use rand::Rng;
use tiny_skia::{BlendMode, Color, Paint, PathBuilder, PixmapMut, Shader, Stroke, Transform};

#[derive(Default, Clone)]
pub struct RingSeg {
    pub off: f64,
    pub len: f64,
    pub r_round_rate: f64,
}

#[derive(Default, Clone)]
pub struct Ring {
    pub ang: f64,
    pub rs: f64,
    pub segs: [RingSeg; 2],
}

#[derive(Default, Clone)]
pub struct Wave {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub life: f64,
    pub ring: Ring,
}

const FILLED_R_ADD_RATE: f64 = 26.0;
const FILLED_MAX_LIFE: f64 = 16.0;
const RINGS_MAX_LIFE: f64 = 23.0;
const RINGS_SEG_NUM: usize = 10;
const RINGS_MIN_W: f64 = 0.4;
const RINGS_MAX_W: f64 = 3.3;
const RINGS_LEN_STOP_ADD: f64 = 0.1;
const RINGS_LEN_START_DIM: f64 = 0.4;

pub struct WaveRenderer {
    pub waves: Vec<Wave>,
    pool: Pool<Wave>,
    rings_start_color: [f32; 3],
    rings_end_color: [f32; 3],
}

impl WaveRenderer {
    pub fn new() -> Self {
        Self {
            waves: Vec::new(),
            pool: Pool::new(),
            rings_start_color: [250.0, 252.0, 252.0],
            rings_end_color: [200.0, 250.0, 255.0],
        }
    }

    pub fn set_color(&mut self, color: [u8; 3], rings_end: [u8; 3]) {
        self.rings_end_color = [
            rings_end[0] as f32,
            rings_end[1] as f32,
            rings_end[2] as f32,
        ];
    }

    pub fn create_wave(&mut self, x: f64, y: f64) {
        let mut wave = self.pool.acquire();
        let rs_list = [0.0, 0.03, 0.06];
        let r_round_list = [0.0, 1.0, 1.5, 2.0];
        let len = 1.1 * std::f64::consts::PI;

        wave.x = x;
        wave.y = y;
        wave.r = 0.0;
        wave.life = 0.0;
        wave.ring.ang = rand::random::<f64>() * std::f64::consts::PI * 2.0;
        let mut rng = rand::rng();
        wave.ring.rs = rs_list[rng.random_range(0..rs_list.len())];
        wave.ring.segs[0] = RingSeg {
            off: 0.0,
            len,
            r_round_rate: r_round_list[rng.random_range(0..r_round_list.len())],
        };
        wave.ring.segs[1] = RingSeg {
            off: (rng.random::<f64>() * 3.0 - 1.5) * std::f64::consts::PI,
            len,
            r_round_rate: r_round_list[rng.random_range(0..r_round_list.len())],
        };

        self.waves.push(wave);
    }

    pub fn render(
        &mut self,
        pixmap: &mut PixmapMut,
        frame_scale: f64,
        color: [u8; 3],
        opacity: f64,
    ) {
        let rings_start = self.rings_start_color;
        let rings_end = self.rings_end_color;

        let mut i: i32 = self.waves.len() as i32 - 1;
        while i >= 0 {
            let idx = i as usize;
            let w = &mut self.waves[idx];
            w.life += frame_scale;

            let wave_prog = (w.life / FILLED_MAX_LIFE).min(1.0);
            let ring_prog = (w.life / RINGS_MAX_LIFE).min(1.0);

            // 作为函数调用避免 borrow checker 冲突
            render_filled_circle(pixmap, &w, wave_prog, color, opacity);
            render_rings(pixmap, &w, ring_prog, opacity, rings_start, rings_end);

            if ring_prog >= 1.0 && wave_prog >= 1.0 {
                let w = self.waves.swap_remove(idx);
                self.pool.release(w);
            }

            i -= 1;
        }
    }

    pub fn has_work(&self) -> bool {
        !self.waves.is_empty()
    }
}

fn render_filled_circle(
    pixmap: &mut PixmapMut,
    w: &Wave,
    wave_prog: f64,
    color: [u8; 3],
    opacity: f64,
) {
    let ease = 1.0 - (1.0 - wave_prog).powi(3);
    let r = FILLED_R_ADD_RATE * ease;
    let alpha = (1.0 - wave_prog).max(0.0) * opacity;

    if alpha > 0.0 && r > 0.0 {
        let paint = Paint {
            shader: Shader::SolidColor(Color::from_rgba8(
                color[0],
                color[1],
                color[2],
                (alpha * 255.0) as u8,
            )),
            blend_mode: BlendMode::Plus,
            ..Default::default()
        };
        if let Some(path) = PathBuilder::from_circle(w.x as f32, w.y as f32, r as f32) {
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
        }
    }
}

fn render_rings(
    pixmap: &mut PixmapMut,
    w: &Wave,
    ring_prog: f64,
    opacity: f64,
    rings_start_color: [f32; 3],
    rings_end_color: [f32; 3],
) {
    let ring_rgb_at = |t: f64| -> [u8; 3] {
        let t = (1.2 * t).min(1.0);
        let t32 = t as f32;
        [
            (rings_start_color[0] * (1.0 - t32) + rings_end_color[0] * t32) as u8,
            (rings_start_color[1] * (1.0 - t32) + rings_end_color[1] * t32) as u8,
            (rings_start_color[2] * (1.0 - t32) + rings_end_color[2] * t32) as u8,
        ]
    };
    let get_alpha = |t: f64| -> f64 { (1.1 - 0.3 * t).min(1.0) * opacity };

    let line_width_mul = (-0.8 * (ring_prog - 0.8) + 1.0).min(1.0);
    let [rr, gg, bb] = ring_rgb_at(ring_prog);
    let alpha_ring = get_alpha(ring_prog);

    if alpha_ring <= 0.0 {
        return;
    }

    for seg in &w.ring.segs {
        let base = w.ring.ang + seg.off + w.ring.rs * w.life;

        let (start, end) = if ring_prog <= RINGS_LEN_STOP_ADD {
            let len = seg.len * (ring_prog / RINGS_LEN_STOP_ADD);
            let e = base + seg.len;
            (e - len, e)
        } else if ring_prog > RINGS_LEN_START_DIM {
            let len =
                seg.len * (1.0 - (ring_prog - RINGS_LEN_START_DIM) / (1.0 - RINGS_LEN_START_DIM));
            (base, base + len)
        } else {
            (base, base + seg.len)
        };

        let radius = w.r + seg.r_round_rate;

        for k in 0..RINGS_SEG_NUM {
            let t0 = k as f64 / RINGS_SEG_NUM as f64;
            let t1 = (k + 1) as f64 / RINGS_SEG_NUM as f64;
            let a0 = start + (end - start) * t0;
            let a1 = start + (end - start) * t1;

            let w_t = (2.0 - (4.0 * (t0 - 0.5)).abs()).min(1.0);
            let lw = (RINGS_MIN_W * (1.0 - w_t) + RINGS_MAX_W * w_t) * line_width_mul;

            if lw <= 0.0 {
                continue;
            }

            let paint = Paint {
                shader: Shader::SolidColor(Color::from_rgba8(
                    rr, gg, bb, (alpha_ring * 255.0) as u8,
                )),
                blend_mode: BlendMode::Plus,
                ..Default::default()
            };

            let stroke = Stroke {
                width: lw as f32,
                ..Default::default()
            };

            // 用线段近似圆弧
            let mut pb = PathBuilder::new();
            let points_on_arc = 4u32;
            let (first_x, first_y) = arc_point(w.x, w.y, radius, a0);
            pb.move_to(first_x as f32, first_y as f32);
            for p in 1..=points_on_arc {
                let t = p as f64 / points_on_arc as f64;
                let angle = a0 + (a1 - a0) * t;
                let (px, py) = arc_point(w.x, w.y, radius, angle);
                pb.line_to(px as f32, py as f32);
            }

            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }
}

fn arc_point(cx: f64, cy: f64, r: f64, angle: f64) -> (f64, f64) {
    (cx + r * angle.cos(), cy + r * angle.sin())
}
