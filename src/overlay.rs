//! Wayland wlr-layer-shell transparent overlay + wl_pointer input

use crate::config::Config;
use crate::input::{InputEvent, MouseButton};
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
        wl_pointer::{self, WlPointer},
        wl_region::WlRegion,
        wl_registry::{self, WlRegistry},
        wl_seat::{self, WlSeat},
        wl_shm::{self, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1},
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
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

pub struct OutputSurface {
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    slots: Vec<BufferSlot>,
    current: usize,
    info: OutputInfo,
    configured: bool,
    pub width: i32,
    pub height: i32,
}

struct AppState {
    running: Arc<AtomicBool>,
    shm: Option<WlShm>,
    compositor: Option<WlCompositor>,
    layer_shell: Option<ZwlrLayerShellV1>,
    outputs: Vec<OutputInfo>,
    surfaces: Vec<OutputSurface>,
    need_redraw: Arc<AtomicBool>,
    // Wayland pointer state
    seat: Option<WlSeat>,
    pointer: Option<WlPointer>,
    vp_manager: Option<ZwlrVirtualPointerManagerV1>,
    virtual_pointer: Option<ZwlrVirtualPointerV1>,
    cursor_x: f64,
    cursor_y: f64,
    cursor_surface_x: f64,
    cursor_surface_y: f64,
    pointer_entered: bool,
    left_down: bool,
    right_down: bool,
    middle_down: bool,
    pending_input: Vec<InputEvent>,
    // 转发计数: 跳过虚拟指针 echo 事件
    forward_skip_buttons: u32,
    // 空 input region (用于临时点击穿透)
    empty_region: Option<WlRegion>,
    // 穿透截止时间
    passthrough_until: Option<std::time::Instant>,
}

pub struct Overlay {
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
        shm: &WlShm,
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
            zwlr_layer_surface_v1::KeyboardInteractivity::OnDemand,
        );

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

    /// 提交当前 buffer 并切到下一个 slot
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
                    registry.bind::<WlOutput, _, _>(name, 3, qh, name);
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind::<ZwlrLayerShellV1, _, _>(name, 1, qh, ()));
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind::<WlSeat, _, _>(name, 7, qh, ()));
                }
                "zwlr_virtual_pointer_manager_v1" => {
                    state.vp_manager = Some(registry.bind::<ZwlrVirtualPointerManagerV1, _, _>(name, 2, qh, ()));
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { .. } => {}
            _ => {}
        }
    }
}

// ── Output dispatch ──

impl Dispatch<WlOutput, u32> for AppState {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: <WlOutput as wayland_client::Proxy>::Event,
        output_id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_output::Event;

        let idx = state.outputs.iter().position(|o| o.name == format!("output-{}", output_id));
        let info = match idx {
            Some(i) => &mut state.outputs[i],
            None => {
                state.outputs.push(OutputInfo { x: 0, y: 0, width: 1920, height: 1080, scale: 1, name: format!("output-{}", output_id) });
                state.outputs.last_mut().unwrap()
            }
        };

        match event {
            Event::Geometry { x, y, .. } => { info.x = x; info.y = y; }
            Event::Mode { width, height, .. } => { info.width = width; info.height = height; }
            Event::Scale { factor } => { info.scale = factor; }
            Event::Name { name } => { info.name = name; }
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
                        // attach first buffer and commit
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

// ── Seat & Pointer dispatch ──

impl Dispatch<WlSeat, ()> for AppState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: <WlSeat as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event {
            if caps.contains(wl_seat::Capability::Pointer) {
                if state.pointer.is_none() {
                    let pointer = seat.get_pointer(qh, ());
                    state.pointer = Some(pointer);
                    log::info!("wl_pointer obtained");
                }
                // 创建 virtual pointer 用于转发
                if let (Some(ref vp_mgr), None) = (&state.vp_manager, &state.virtual_pointer) {
                    let vp = vp_mgr.create_virtual_pointer(Some(seat), qh, ());
                    state.virtual_pointer = Some(vp);
                    log::info!("zwlr_virtual_pointer_v1 created for forwarding");
                }
            }
        }
    }
}

impl Dispatch<WlPointer, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: <WlPointer as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { surface_x, surface_y, .. } => {
                state.pointer_entered = true;
                state.cursor_surface_x = surface_x;
                state.cursor_surface_y = surface_y;
                state.cursor_x = surface_x;
                state.cursor_y = surface_y;
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                if !state.pointer_entered { return; }
                state.cursor_surface_x = surface_x;
                state.cursor_surface_y = surface_y;
                state.cursor_x = surface_x;
                state.cursor_y = surface_y;
                if state.left_down || state.right_down || state.middle_down {
                    state.pending_input.push(InputEvent::MouseMove { x: surface_x, y: surface_y });
                }
            }
            wl_pointer::Event::Button { serial, time, button, state: WEnum::Value(btn_state) } => {
                let btn = match button {
                    0x110 => MouseButton::Left,
                    0x111 => MouseButton::Right,
                    0x112 => MouseButton::Middle,
                    _ => return,
                };

                // 跳过虚拟指针 echo
                if state.forward_skip_buttons > 0 {
                    state.forward_skip_buttons -= 1;
                    return;
                }

                if btn_state == wl_pointer::ButtonState::Pressed {
                    match btn {
                        MouseButton::Left => state.left_down = true,
                        MouseButton::Right => state.right_down = true,
                        MouseButton::Middle => state.middle_down = true,
                    }
                    state.pending_input.push(InputEvent::MouseDown {
                        button: btn,
                        x: state.cursor_x,
                        y: state.cursor_y,
                    });

                    // 转发到虚拟指针: 先穿透再转发
                    if let Some(ref vp) = state.virtual_pointer {
                        Self::enable_passthrough(&mut state.surfaces, &state.empty_region);
                        vp.button(time, button, wl_pointer::ButtonState::Pressed);
                        vp.frame();
                        state.forward_skip_buttons += 1;
                        state.passthrough_until = Some(std::time::Instant::now() + std::time::Duration::from_millis(100));
                    }
                } else {
                    match btn {
                        MouseButton::Left => state.left_down = false,
                        MouseButton::Right => state.right_down = false,
                        MouseButton::Middle => state.middle_down = false,
                    }
                    state.pending_input.push(InputEvent::MouseUp { button: btn });

                    // 转发释放事件
                    if let Some(ref vp) = state.virtual_pointer {
                        Self::enable_passthrough(&mut state.surfaces, &state.empty_region);
                        vp.button(time, button, wl_pointer::ButtonState::Released);
                        vp.frame();
                        state.forward_skip_buttons += 1;
                        state.passthrough_until = Some(std::time::Instant::now() + std::time::Duration::from_millis(100));
                    }
                }
            }
            wl_pointer::Event::Leave { .. } => {
                state.pointer_entered = false;
            }
            _ => {}
        }
    }
}

// ── Helpers ──

impl AppState {
    fn enable_passthrough(surfaces: &mut [OutputSurface], empty_region: &Option<WlRegion>) {
        for s in surfaces {
            s.surface.set_input_region(empty_region.as_ref());
            s.surface.commit();
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
delegate_noop!(AppState: ignore ZwlrVirtualPointerManagerV1);
delegate_noop!(AppState: ignore ZwlrVirtualPointerV1);

// ── Overlay public API ──

impl Overlay {
    pub fn new(_config: &Config) -> Result<Self, Box<dyn std::error::Error>> {
        log::info!("Connecting to Wayland compositor...");
        let conn = Connection::connect_to_env()?;
        let event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let running = Arc::new(AtomicBool::new(true));
        let need_redraw = Arc::new(AtomicBool::new(false));

        conn.display().get_registry(&qhandle, ());

        // 等 roundtrip 后 compositor 才会绑定...
        // 暂时先初始化 state

        let state = AppState {
            running: running.clone(),
            shm: None,
            compositor: None,
            layer_shell: None,
            outputs: vec![],
            surfaces: vec![],
            need_redraw: need_redraw.clone(),
            seat: None,
            pointer: None,
            vp_manager: None,
            virtual_pointer: None,
            cursor_x: 960.0,
            cursor_y: 540.0,
            cursor_surface_x: 960.0,
            cursor_surface_y: 540.0,
            pointer_entered: false,
            left_down: false,
            right_down: false,
            middle_down: false,
            pending_input: Vec::new(),
            forward_skip_buttons: 0,
            empty_region: None,
            passthrough_until: None,
        };

        Ok(Overlay { state, conn, event_queue, qhandle, running, need_redraw })
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Performing initial Wayland roundtrips...");
        self.event_queue.roundtrip(&mut self.state)?;
        self.event_queue.roundtrip(&mut self.state)?;

        // 创建空 input region 用于临时点击穿透
        if let Some(ref compositor) = self.state.compositor {
            let region = compositor.create_region(&self.qhandle, ());
            self.state.empty_region = Some(region);
        }

        self.create_surfaces()?;
        self.event_queue.roundtrip(&mut self.state)?;
        self.event_queue.roundtrip(&mut self.state)?;
        log::info!("Overlay initialized with {} surface(s), pointer_entered={}",
            self.state.surfaces.len(), self.state.pointer_entered);
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

    /// dispatch + 读取 socket
    /// block=true: 空闲时循环等待事件
    /// block=false: 活跃时非阻塞尝试读取
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

    /// 检查点击穿透超时，恢复 input region
    pub fn check_passthrough(&mut self) {
        if let Some(until) = self.state.passthrough_until {
            if std::time::Instant::now() >= until {
                for s in &mut self.state.surfaces {
                    s.surface.set_input_region(None);
                    s.surface.commit();
                }
                self.state.passthrough_until = None;
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.state.running.load(Ordering::Relaxed)
    }

    pub fn surfaces_mut(&mut self) -> &mut Vec<OutputSurface> {
        &mut self.state.surfaces
    }

    /// 取出所有待处理的 Wayland 输入事件
    pub fn take_input_events(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.state.pending_input)
    }

    pub fn is_button_down(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => self.state.left_down,
            MouseButton::Right => self.state.right_down,
            MouseButton::Middle => self.state.middle_down,
        }
    }

    pub fn commit_and_swap(&mut self) {
        for surface in &mut self.state.surfaces {
            surface.commit_and_swap();
        }
    }
}
