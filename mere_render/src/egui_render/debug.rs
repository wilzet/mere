use crate::egui_render::EguiRenderer;
use egui_plot::{Line, Plot, PlotPoints};
use mere_asset::World;
use std::time::Duration;
use winit::window::Window;

pub trait DebugWindow {
    fn debug_window(&mut self, window: &Window, world: &World, delta_time: Duration);
}

impl DebugWindow for EguiRenderer {
    fn debug_window(&mut self, window: &Window, world: &World, delta_time: Duration) {
        let (width, height): (f64, f64) = window.inner_size().into();
        let frame_time_ms = delta_time.as_secs_f32() * 1000.0;
        let fps = 1.0 / delta_time.as_secs_f32();

        self.fps_history.push_back(fps);
        if self.fps_history.len() > 200 {
            self.fps_history.pop_front();
        }

        let mut new_scale_factor = self.scale_factor;

        egui::Window::new("🚀 Engine Diagnostics")
            .resizable(true)
            .vscroll(true)
            .default_width(300.0)
            .show(self.context(), |ui| {
                // --- PERFORMANCE SECTION ---
                ui.heading("Performance");
                let avg_fps: f32 =
                    self.fps_history.iter().sum::<f32>() / self.fps_history.len() as f32;

                ui.horizontal(|ui| {
                    // Use a color based on the AVERAGE performance for a more stable UI
                    let status_color = if avg_fps > 55.0 {
                        egui::Color32::from_rgb(100, 255, 100) // Healthy Green
                    } else if avg_fps > 30.0 {
                        egui::Color32::GOLD
                    } else {
                        egui::Color32::from_rgb(255, 100, 100) // Stressful Red
                    };

                    ui.visuals_mut().override_text_color = Some(status_color);

                    // Display both: Instant (jittery) and Average (stable)
                    ui.label(format!("FPS: {:.0} (avg: {:.0})", fps, avg_fps));
                    ui.label(format!("| {:.02} ms", frame_time_ms));
                });

                // Sparkline-style plot to see performance drops over time
                let points: PlotPoints = self
                    .fps_history
                    .iter()
                    .enumerate()
                    .map(|(i, f)| [i as f64, *f as f64])
                    .collect();
                let line = Line::new("FPS", points).fill(0.0);

                Plot::new("fps_plot")
                    .view_aspect(4.0)
                    .show_axes([false, true])
                    .allow_drag(false)
                    .allow_scroll(false)
                    .allow_zoom(false)
                    .show(ui, |plot_ui| plot_ui.line(line));

                ui.separator();

                // --- MESHLET / SCENE STATS ---
                ui.heading("Scene Stats");

                ui.label(format!(
                    "Active Instances: {}",
                    world.instances().scene_instance_count
                ));
                ui.label(format!(
                    "Total Meshlets:   {}",
                    world.instances().count_clusters()
                ));

                ui.collapsing("Hierarchy Explorer", |ui| {
                    for (i, instance) in world.iter_instances().enumerate() {
                        let model_name = world
                            .get_meshlet_mesh(instance.meshlet)
                            .map(|m| m.read().name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());

                        ui.collapsing(format!("{}: {}", i, model_name), |ui| {
                            ui.monospace(format!("Pos: {:.2?}", instance.transform.translation));
                            ui.monospace(format!(
                                "Rot: {:.2?}",
                                instance.transform.rotation.to_euler(Default::default())
                            ));
                        });
                    }
                });

                ui.separator();

                // --- SYSTEM & UI ---
                ui.collapsing("System Settings", |ui| {
                    ui.label(format!("Resolution: {}x{}", width, height));
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "UI Scale: {:.1}",
                            self.context().pixels_per_point()
                        ));
                        if ui.button("-").clicked() {
                            new_scale_factor -= 0.1;
                        }
                        if ui.button("+").clicked() {
                            new_scale_factor += 0.1;
                        }
                    });
                });
            });

        self.scale_factor = new_scale_factor.clamp(0.3, 3.0);
    }
}
