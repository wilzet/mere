use mere_render::State;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler, event::*, event_loop::ActiveEventLoop, window::Window,
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
                state.request_redraw();
            }
            _ => state.handle_input(event_loop, &event),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        mere_log::info!("Shutting down...")
    }
}
