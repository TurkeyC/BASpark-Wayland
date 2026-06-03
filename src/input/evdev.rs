use evdev::{Device, EventType, InputEvent as EvdevInputEvent, RelativeAxisCode};
use std::os::unix::io::AsRawFd;

use super::cursor::{CursorTracker, MouseButton};
use super::InputEvent;

use libc::{epoll_event, EPOLLIN};

pub struct EvdevMonitor {
    devices: Vec<Device>,
    pub cursor: CursorTracker,
    epoll_fd: Option<i32>,
}

impl EvdevMonitor {
    pub fn new(cursor: CursorTracker) -> Self {
        Self {
            devices: Vec::new(),
            cursor,
            epoll_fd: None,
        }
    }

    pub fn discover(&mut self) {
        let dir = std::path::Path::new("/dev/input");
        let mut found = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(device) = Device::open(&path) {
                    if is_pointer_device(&device) {
                        let name = device.name().unwrap_or("unknown").to_string();
                        let _ = device.set_nonblocking(true);
                        log::info!("Found pointer device: {} at {:?}", name, path);
                        found.push(device);
                    }
                }
            }
        }

        // 只使用第一个设备，多设备会导致定位混乱
        if found.len() > 1 {
            log::info!("Multiple devices found, using first: {:?}", found[0].name());
            self.devices = vec![found.swap_remove(0)];
            // 关闭其余设备的 fd（drop 会自动关闭）
        } else {
            self.devices = found;
        }

        if self.devices.is_empty() {
            log::warn!("No pointer devices found (try adding user to 'input' group)");
        } else {
            self.setup_epoll();
        }
    }

    fn setup_epoll(&mut self) {
        let epfd = match epoll_create() {
            Ok(fd) => fd,
            Err(e) => {
                log::warn!("Failed to create epoll fd: {}", e);
                return;
            }
        };

        for device in &self.devices {
            let fd = device.as_raw_fd();
            let mut ev = epoll_event {
                events: EPOLLIN as u32,
                u64: fd as u64,
            };
            if epoll_ctl(epfd, libc::EPOLL_CTL_ADD, fd, &mut ev) < 0 {
                log::warn!("Failed to add fd {} to epoll", fd);
            }
        }

        self.epoll_fd = Some(epfd);
    }

    /// 阻塞等待 evdev 事件，最多等待 timeout_ms 毫秒。
    /// 返回 true 表示有事件可读，false 表示超时。
    pub fn wait_for_events(&self, timeout_ms: i32) -> bool {
        let epfd = match self.epoll_fd {
            Some(fd) => fd,
            None => return false,
        };

        let mut ev = epoll_event { events: 0, u64: 0 };
        let ret = epoll_wait(epfd, &mut ev, 1, timeout_ms);
        ret > 0
    }

    /// 非阻塞读取所有待处理的 evdev 事件
    pub fn poll(&mut self) -> Vec<InputEvent> {
        let mut batch = Vec::new();
        for device in &mut self.devices {
            if let Ok(events) = device.fetch_events() {
                for ev in events {
                    batch.push(ev);
                }
            }
        }

        let mut result = Vec::new();
        for ev in batch {
            self.process_event(ev, &mut result);
        }
        result
    }

    fn process_event(
        &mut self,
        ev: EvdevInputEvent,
        events: &mut Vec<InputEvent>,
    ) {
        if ev.event_type() == EventType::RELATIVE {
            let code = ev.code();
            let value = ev.value();
            match RelativeAxisCode::from(RelativeAxisCode(code)) {
                RelativeAxisCode::REL_X => {
                    self.cursor.apply_delta(value as f64, 0.0);
                    if self.cursor.is_down {
                        events.push(self.cursor.to_move_event());
                    }
                }
                RelativeAxisCode::REL_Y => {
                    self.cursor.apply_delta(0.0, value as f64);
                    if self.cursor.is_down {
                        events.push(self.cursor.to_move_event());
                    }
                }
                _ => {}
            }
            return;
        }

        if ev.event_type() == EventType::KEY {
            let code = ev.code();
            let pressed = ev.value() != 0;

            let button = match code {
                0x110 => Some(MouseButton::Left),
                0x111 => Some(MouseButton::Right),
                0x112 => Some(MouseButton::Middle),
                _ => None,
            };

            if let Some(btn) = button {
                self.cursor.set_button(btn, pressed);
                events.push(self.cursor.to_event_on_click(btn, pressed));
            }
        }
    }

    pub fn has_devices(&self) -> bool {
        !self.devices.is_empty()
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Drop for EvdevMonitor {
    fn drop(&mut self) {
        if let Some(fd) = self.epoll_fd {
            unsafe { libc::close(fd); }
        }
    }
}

fn is_pointer_device(device: &Device) -> bool {
    let has_rel = device
        .supported_events()
        .contains(EventType::RELATIVE);
    let has_rel_axes = device
        .supported_relative_axes()
        .map_or(false, |axes| {
            axes.contains(RelativeAxisCode::REL_X)
                && axes.contains(RelativeAxisCode::REL_Y)
        });
    let has_keys = device.supported_events().contains(EventType::KEY);

    has_rel && has_rel_axes && has_keys
}

fn epoll_create() -> Result<i32, std::io::Error> {
    let ret = unsafe { libc::epoll_create1(0) };
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(ret)
    }
}

fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: &mut epoll_event) -> i32 {
    unsafe { libc::epoll_ctl(epfd, op, fd, event) }
}

fn epoll_wait(epfd: i32, events: &mut epoll_event, maxevents: i32, timeout: i32) -> i32 {
    unsafe { libc::epoll_wait(epfd, events, maxevents, timeout) }
}
