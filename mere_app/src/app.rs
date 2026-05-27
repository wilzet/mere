use mere_render::State;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler, event::*, event_loop::ActiveEventLoop, window::Window,
};

/// Main application handler driven by the [`winit`] event loop.
///
/// # Notes
/// - [`state`](Self::state) is [`None`] until [`resumed`](App::resumed) is called
/// - The application exits if initialization fails
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
        let window = match event_loop.create_window(window_attributes) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                mere_log::error!("Failed to create window: {err}");
                event_loop.exit();
                return;
            }
        };

        self.state = match pollster::block_on(State::new(window)) {
            Ok(state) => Some(state),
            Err(err) => {
                mere_log::error!("Failed to initialize state: {err}");
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
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now - self.last_frame_time;
                self.last_frame_time = now;
                state.update(dt);

                if let Err(err) = state.render(dt) {
                    mere_log::error!("Render error: {err}");
                    event_loop.exit();
                    return;
                }

                state.after_render();
                state.request_redraw();
            }
            _ => state.handle_input(event_loop, &event),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        mere_log::info!("Shutting down...");
    }
}
