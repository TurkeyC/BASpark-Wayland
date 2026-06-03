/// 泛型对象池 — 对应原 JS 的 sparksPool / wavesPool
pub struct Pool<T: Default> {
    inactive: Vec<T>,
}

impl<T: Default> Pool<T> {
    pub fn new() -> Self {
        Self {
            inactive: Vec::new(),
        }
    }

    /// 获取一个对象 (优先从池中复用)
    pub fn acquire(&mut self) -> T {
        self.inactive.pop().unwrap_or_default()
    }

    /// 归还对象到池中
    pub fn release(&mut self, item: T) {
        self.inactive.push(item);
    }
}
