use crate::egui_render::EguiRenderer;
use mere_asset::World;
use std::time::Duration;
use winit::window::Window;

pub trait DebugWindow {
    fn debug_window(&mut self, window: &Window, world: &World, delta_time: Duration);
}

impl DebugWindow for EguiRenderer {
    fn debug_window(&mut self, window: &Window, world: &World, delta_time: Duration) {
        let (width, height): (f64, f64) = window.inner_size().into();

        let mut new_scale_factor = self.scale_factor;
        egui::Window::new(format!("Debug {width}x{height}"))
            .resizable(true)
            .vscroll(true)
            .default_open(true)
            .show(self.context(), |ui| {
                let frame_time = delta_time.as_secs_f32();
                let fps = (1.0 / frame_time) as u32;
                ui.label(format!("{:.01} ms / {:>4} fps", frame_time * 1000.0, fps));

                ui.collapsing("Scene", |ui| {
                    ui.collapsing("Objects", |ui| {
                        for instance in world.instances() {
                            let model = world.get_meshlet_mesh(instance.meshlet).unwrap();
                            ui.label(format!(
                                "{}\n\tposition: {}\n\trotation: {:?}",
                                model.read().name,
                                instance.transform.translation,
                                instance.transform.rotation.to_euler(Default::default())
                            ));
                        }
                    });

                    let camera = world.main_camera();
                    ui.label(format!(
                        "Main Camera\n\tposition: {}\n\trotation: {:?}",
                        camera.transform.translation,
                        camera.transform.rotation.to_euler(Default::default())
                    ));
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Pixels per point: {}",
                        self.context().pixels_per_point()
                    ));
                    if ui.button("-").clicked() {
                        new_scale_factor = (self.scale_factor - 0.1).max(0.3);
                    }
                    if ui.button("+").clicked() {
                        new_scale_factor = (self.scale_factor + 0.1).min(3.0);
                    }
                });
            });
        self.scale_factor = new_scale_factor;
    }
}
