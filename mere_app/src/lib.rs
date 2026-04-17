use mere_render::State;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::PhysicalKey,
    window::Window,
};

pub struct App {
    state: Option<State>,
    last_frame_time: Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None,
            last_frame_time: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title("MeRe");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.state = match pollster::block_on(State::new(window)) {
            Ok(state) => Some(state),
            Err(err) => {
                mere_log::error!("{err}");
                event_loop.exit();
                None
            }
        };
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(state) => state,
            None => return,
        };

        let ui_input = state.egui_renderer.handle_input(&state.window, &event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now - self.last_frame_time;
                self.last_frame_time = now;
                state.update(dt);
                if let Err(err) = state.render(dt) {
                    mere_log::error!("{err}");
                    event_loop.exit();
                }
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed()),
            WindowEvent::CursorMoved { position, .. } => {
                state.handle_mouse_moved(position.x, position.y)
            }
            WindowEvent::MouseInput {
                state: key_state,
                button,
                ..
            } if !ui_input => state.handle_mouse_input(button, key_state.is_pressed()),
            WindowEvent::MouseWheel { delta, .. } => state.handle_mouse_scroll(delta),
            _ => (),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        mere_log::info!("Shutting down...")
    }
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::builder().build()?;
    let mut app = App::new();

    match event_loop.run_app(&mut app) {
        Ok(_) => (),
        Err(err) => mere_log::error!(return err),
    }

    Ok(())
}
