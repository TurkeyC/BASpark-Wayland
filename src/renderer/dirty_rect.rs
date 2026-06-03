/// 脏矩形 — 对应原 JS 的 dirty rect 系统
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    fn left(&self) -> f64 {
        self.x
    }
    fn right(&self) -> f64 {
        self.x + self.w
    }
    fn top(&self) -> f64 {
        self.y
    }
    fn bottom(&self) -> f64 {
        self.y + self.h
    }
    fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
}

/// 脏矩形追踪器 — 对应原 JS _getEffectRects / _mergeRects / _clipToRects
pub struct DirtyRectTracker {
    previous_rects: Vec<Rect>,
}

impl DirtyRectTracker {
    pub fn new() -> Self {
        Self {
            previous_rects: Vec::new(),
        }
    }

    /// 保存脏矩形供下一帧使用
    pub fn set_dirty_rects(&mut self, rects: Vec<Rect>) {
        self.previous_rects = rects;
    }

    /// 获取渲染矩形 (上一帧脏区域 + 当前帧效果区域)
    pub fn get_render_rects(&self, effect_rects: &[Rect]) -> Vec<Rect> {
        let mut all = self.previous_rects.clone();
        all.extend_from_slice(effect_rects);
        Self::merge_rects(&all)
    }

    /// 合并相交矩形 — 对应原 JS _mergeRects
    fn merge_rects(rects: &[Rect]) -> Vec<Rect> {
        let mut merged: Vec<Rect> = Vec::new();

        for &raw in rects {
            if raw.is_empty() {
                continue;
            }
            let mut rect = raw;
            let mut i = 0;
            while i < merged.len() {
                if intersects(&rect, &merged[i]) {
                    rect = union_rect(&rect, &merged[i]);
                    merged.swap_remove(i);
                    i = 0;
                } else {
                    i += 1;
                }
            }
            merged.push(rect);
        }

        merged
    }
}

fn intersects(a: &Rect, b: &Rect) -> bool {
    a.left() <= b.right() && a.right() >= b.left() && a.top() <= b.bottom() && a.bottom() >= b.top()
}

fn union_rect(a: &Rect, b: &Rect) -> Rect {
    let x0 = a.left().min(b.left());
    let y0 = a.top().min(b.top());
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

pub fn point_rect(x: f64, y: f64, padding: f64) -> Rect {
    Rect::new(x - padding, y - padding, padding * 2.0, padding * 2.0)
}

pub fn segment_rect(a: (f64, f64), b: (f64, f64), padding: f64) -> Rect {
    let x0 = a.0.min(b.0) - padding;
    let y0 = a.1.min(b.1) - padding;
    let x1 = a.0.max(b.0) + padding;
    let y1 = a.1.max(b.1) + padding;
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}
