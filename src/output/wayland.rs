use memmap2::MmapMut;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_callback::WlCallback,
        wl_compositor::WlCompositor,
        wl_output::WlOutput,
        wl_registry::{self, WlRegistry},
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_region::WlRegion,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1},
};

#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: i32,
    pub name: String,
}

struct BufferSlot {
    fd: OwnedFd,
    pool: WlShmPool,
    buffer: WlBuffer,
    mmap: MmapMut,
}

pub(crate) struct OutputSurface {
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    slots: Vec<BufferSlot>,
    current: usize,
    info: OutputInfo,
    configured: bool,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

struct AppState {
    running: Arc<AtomicBool>,
    shm: Option<WlShm>,
    compositor: Option<WlCompositor>,
    layer_shell: Option<ZwlrLayerShellV1>,
    outputs: Vec<OutputInfo>,
    surfaces: Vec<OutputSurface>,
    need_redraw: Arc<AtomicBool>,
}

pub struct WaylandOutput {
    state: AppState,
    conn: Connection,
    event_queue: wayland_client::EventQueue<AppState>,
    qhandle: QueueHandle<AppState>,
    running: Arc<AtomicBool>,
    need_redraw: Arc<AtomicBool>,
}

const NUM_BUFFERS: usize = 2;

impl OutputSurface {
    fn new(
        compositor: &WlCompositor,
        _shm: &WlShm,
        layer_shell: &ZwlrLayerShellV1,
        info: OutputInfo,
        qh: &QueueHandle<AppState>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let surface = compositor.create_surface(qh, ());

        let layer_surface = layer_shell.get_layer_surface(
            &surface, None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "baspark".to_string(), qh, (),
        );

        layer_surface.set_anchor(
            zwlr_layer_surface_v1::Anchor::Top
                | zwlr_layer_surface_v1::Anchor::Bottom
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(
            zwlr_layer_surface_v1::KeyboardInteractivity::None,
        );

        // 设置空 input region 以实现鼠标点击穿透
        let empty_region = compositor.create_region(qh, ());
        surface.set_input_region(Some(&empty_region));

        surface.commit();

        Ok(OutputSurface {
            surface,
            layer_surface,
            slots: Vec::new(),
            current: 0,
            info,
            configured: false,
            width: 1,
            height: 1,
        })
    }

    fn alloc_buffers(&mut self, shm: &WlShm, qh: &QueueHandle<AppState>) -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..NUM_BUFFERS {
            let (fd, pool, mmap, _w, _h) = create_shm_buffer(shm, self.width, self.height, qh)?;
            let buffer = pool.create_buffer(0, self.width, self.height, self.width * 4, wl_shm::Format::Argb8888, qh, ());
            self.slots.push(BufferSlot { fd, pool, buffer, mmap });
        }
        Ok(())
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.slots[self.current].mmap[..]
    }

    pub fn commit_and_swap(&mut self) {
        let slot = &self.slots[self.current];
        self.surface.attach(Some(&slot.buffer), 0, 0);
        self.surface.commit();
        self.current = (self.current + 1) % self.slots.len();
    }
}

fn create_shm_buffer(
    shm: &WlShm,
    width: i32,
    height: i32,
    qh: &QueueHandle<AppState>,
) -> Result<(OwnedFd, WlShmPool, MmapMut, i32, i32), Box<dyn std::error::Error>> {
    let size = (width * 4 * height) as usize;
    let file = tempfile::tempfile()?;
    file.set_len(size as u64)?;
    let fd = OwnedFd::from(file);
    let mmap = unsafe { MmapMut::map_mut(fd.as_raw_fd())? };
    let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());
    Ok((fd, pool, mmap, width, height))
}

// ── Registry dispatch ──

impl Dispatch<WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, .. } => match &interface[..] {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind::<WlCompositor, _, _>(name, 1, qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind::<WlShm, _, _>(name, 1, qh, ()));
                }
                "wl_output" => {
                    registry.bind::<WlOutput, _, _>(name, 4, qh, ());
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind::<ZwlrLayerShellV1, _, _>(name, 1, qh, ()));
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { .. } => {}
            _ => {}
        }
    }
}

// ── Output dispatch ──

impl Dispatch<WlOutput, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: <WlOutput as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_output::Event;

        match event {
            Event::Geometry { x, y, .. } => {
                let idx = state.outputs.iter().position(|o| o.x == x && o.y == y);
                if let Some(i) = idx {
                    state.outputs[i].x = x;
                    state.outputs[i].y = y;
                } else {
                    state.outputs.push(OutputInfo { x, y, width: 1920, height: 1080, scale: 1, name: String::new() });
                }
            }
            Event::Mode { width, height, .. } => {
                if let Some(info) = state.outputs.last_mut() {
                    info.width = width;
                    info.height = height;
                }
            }
            Event::Scale { factor } => {
                if let Some(info) = state.outputs.last_mut() {
                    info.scale = factor;
                }
            }
            Event::Name { name } => {
                // 更新最后一个 output 的名称
                if let Some(info) = state.outputs.last_mut() {
                    info.name = name;
                }
            }
            Event::Done => {}
            _ => {}
        }
    }
}

// ── Layer shell dispatch ──

impl Dispatch<ZwlrLayerShellV1, ()> for AppState {
    fn event(_: &mut Self, _: &ZwlrLayerShellV1, _: <ZwlrLayerShellV1 as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event;

        match event {
            Event::Configure { serial, width, height } => {
                layer_surface.ack_configure(serial);
                if width == 0 || height == 0 { return; }

                for s in &mut state.surfaces {
                    if s.configured { continue; }
                    s.configured = true;
                    s.width = width as i32;
                    s.height = height as i32;
                    log::info!("Layer surface configured: {}x{}", width, height);

                    if let Some(ref shm) = state.shm {
                        let _ = s.alloc_buffers(shm, qh);
                        if !s.slots.is_empty() {
                            s.surface.attach(Some(&s.slots[0].buffer), 0, 0);
                            s.surface.commit();
                        }
                    }
                    break;
                }
            }
            Event::Closed => { state.running.store(false, Ordering::Relaxed); }
            _ => {}
        }
    }
}

// ── No-op delegates ──

delegate_noop!(AppState: ignore WlCompositor);
delegate_noop!(AppState: ignore WlShm);
delegate_noop!(AppState: ignore WlShmPool);
delegate_noop!(AppState: ignore WlBuffer);
delegate_noop!(AppState: ignore WlSurface);
delegate_noop!(AppState: ignore WlCallback);
delegate_noop!(AppState: ignore WlRegion);

// ── WaylandOutput public API ──

impl WaylandOutput {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        log::info!("Connecting to Wayland compositor...");
        let conn = Connection::connect_to_env()?;
        let event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let running = Arc::new(AtomicBool::new(true));
        let need_redraw = Arc::new(AtomicBool::new(false));

        conn.display().get_registry(&qhandle, ());

        let state = AppState {
            running: running.clone(),
            shm: None,
            compositor: None,
            layer_shell: None,
            outputs: vec![],
            surfaces: vec![],
            need_redraw: need_redraw.clone(),
        };

        Ok(WaylandOutput { state, conn, event_queue, qhandle, running, need_redraw })
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Performing initial Wayland roundtrips...");
        self.event_queue.roundtrip(&mut self.state)?;
        self.event_queue.roundtrip(&mut self.state)?;

        self.create_surfaces()?;
        self.event_queue.roundtrip(&mut self.state)?;
        self.event_queue.roundtrip(&mut self.state)?;
        log::info!("Wayland output initialized with {} surface(s)",
            self.state.surfaces.len());
        Ok(())
    }

    fn create_surfaces(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let compositor = self.state.compositor.as_ref().ok_or("No wl_compositor")?;
        let shm = self.state.shm.as_ref().ok_or("No wl_shm")?;
        let layer_shell = self.state.layer_shell.as_ref().ok_or("No zwlr_layer_shell_v1")?;

        for info in &self.state.outputs {
            log::info!("Creating layer surface for output {} ({}x{}+{}+{})", info.name, info.width, info.height, info.x, info.y);
            let surface = OutputSurface::new(compositor, shm, layer_shell, info.clone(), &self.qhandle)?;
            self.state.surfaces.push(surface);
        }
        Ok(())
    }

    pub fn dispatch_pending(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(self.event_queue.dispatch_pending(&mut self.state)?)
    }

    pub fn poll_events(&mut self, block: bool) -> Result<usize, Box<dyn std::error::Error>> {
        loop {
            let n = self.event_queue.dispatch_pending(&mut self.state)?;
            if n > 0 {
                return Ok(n);
            }

            self.conn.flush()?;

            if let Some(guard) = self.conn.prepare_read() {
                match guard.read() {
                    Ok(n) => {
                        if n > 0 {
                            return Ok(n + self.event_queue.dispatch_pending(&mut self.state)?);
                        }
                    }
                    Err(_would_block) => {
                        if !block {
                            return Ok(0);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            } else if !block {
                return Ok(0);
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.state.running.load(Ordering::Relaxed)
    }

    pub(crate) fn surfaces_mut(&mut self) -> &mut Vec<OutputSurface> {
        &mut self.state.surfaces
    }

    pub fn commit_and_swap(&mut self) {
        for surface in &mut self.state.surfaces {
            surface.commit_and_swap();
        }
    }
}
