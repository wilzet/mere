//! Application runtime and event loop.
//!
//! This crate provides the entry point for running an application using
//! the [`winit`] event loop. It is responsible for initializing the runtime
//! environment and delegating execution to the [`App`](app::App) type.

use winit::event_loop::EventLoop;

mod app;

/// Starts the application event loop.
///
/// This function:
/// - Initializes the global logger
/// - Creates the window/event loop
/// - Runs the main application lifecycle
///
/// # Errors
/// Returns an error if:
/// - The event loop fails to initialize
/// - The application exits with an error
///
/// # Notes
/// This function blocks until the application exits.
pub fn run() -> anyhow::Result<()> {
    // Initialize logger (must be done once at program start)
    env_logger::init();

    let event_loop = EventLoop::builder().build()?;
    let mut app = app::App::new();

    if let Err(err) = event_loop.run_app(&mut app) {
        mere_log::error!(return err);
    }

    Ok(())
}
