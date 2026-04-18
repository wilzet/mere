use winit::event_loop::EventLoop;

mod app;

pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::builder().build()?;
    let mut app = app::App::new();

    match event_loop.run_app(&mut app) {
        Ok(_) => (),
        Err(err) => mere_log::error!(return err),
    }

    Ok(())
}
