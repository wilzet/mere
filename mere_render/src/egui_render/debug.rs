use super::profiler::{Profiler, ResolvedSpan};
use egui_plot::{Line, Plot, PlotPoints};
use mere_asset::World;
use std::{collections::VecDeque, time::Duration};
use winit::window::Window;

const FRAME_UPDATE_TIME: f32 = 0.8;

pub struct DebugMemory {
    pub profiler: Profiler,
    scale_factor: f32,
    fps_history: VecDeque<f32>,
    fps_long_history: VecDeque<f32>,
    sample_timer: f32,
    accumulator: f64,
    frame_count: u32,
    cpu_history: VecDeque<Vec<ResolvedSpan>>,
    gpu_history: VecDeque<Vec<ResolvedSpan>>,
}

impl DebugMemory {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            profiler: Profiler::new(device),
            scale_factor: 1.0,
            fps_history: VecDeque::with_capacity(1001),
            fps_long_history: VecDeque::with_capacity(3601),
            sample_timer: 0.0,
            accumulator: 0.0,
            frame_count: 0,
            cpu_history: VecDeque::with_capacity(101),
            gpu_history: VecDeque::with_capacity(101),
        }
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn update_history(&mut self, fps: f32, delta_time_secs: f32) {
        self.fps_history.push_back(fps);
        if self.fps_history.len() > 1000 {
            self.fps_history.pop_front();
        }

        self.accumulator += fps as f64;
        self.frame_count += 1;
        self.sample_timer += delta_time_secs;

        if self.sample_timer >= FRAME_UPDATE_TIME {
            let avg_this_period = self.accumulator / self.frame_count as f64;
            self.fps_long_history.push_back(avg_this_period as f32);
            if self.fps_long_history.len() > 3600 {
                self.fps_long_history.pop_front();
            }
            self.sample_timer = 0.0;
            self.accumulator = 0.0;
            self.frame_count = 0;
        }
    }

    fn avg_fps(&self) -> Option<f32> {
        self.fps_long_history.back().copied()
    }
}

pub fn debugger(
    debug_memory: &mut DebugMemory,
    device: &wgpu::Device,
    ctx: &egui::Context,
    window: &Window,
    world: &World,
    delta_time: Duration,
    update_view: &mut bool,
) {
    let (width, height): (f64, f64) = window.inner_size().into();
    let dt = delta_time.as_secs_f32();
    let frame_time_ms = dt * 1000.0;
    let fps = 1.0 / dt;

    debug_memory.update_history(fps, dt);
    let avg_fps = debug_memory.avg_fps().unwrap_or(fps);
    let mut new_scale_factor = debug_memory.scale_factor;

        egui::Area::new(egui::Id::new("Debug Controls"))
            .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
            .show(&ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(egui::Color32::from_black_alpha(150))
                    .shadow(egui::Shadow::NONE)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            ui.label(
                                egui::RichText::new(format!("{:.0} FPS", avg_fps))
                                    .color(egui::Color32::LIGHT_GREEN)
                                    .strong(),
                            );

                            ui.separator();

                            if ui.input(|i| i.key_pressed(egui::Key::L)) {
                                *update_view = !*update_view;
                            }

                            let icon = if *update_view { "🔄" } else { "🔒" };
                            ui.checkbox(update_view, format!("{} View", icon));
                        })
                    });
            });

    egui::Window::new("MeRe Engine Debugger")
        .resizable(true)
        .default_width(340.0)
        .default_height(450.0)
        .show(ctx, |ui| {
            // --- FOOTER ---
            egui::Panel::bottom("debug_footer").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.weak(format!("Res: {}x{}", width, height));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+").clicked() {
                            new_scale_factor += 0.1;
                        }
                        ui.monospace(format!("{:.1}", debug_memory.scale_factor));
                        if ui.button("-").clicked() {
                            new_scale_factor -= 0.1;
                        }
                        ui.label("UI Scale");
                    });
                });
            });

            // --- MAIN CONTENT ---
            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("main_debug_scroll")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        draw_perf_section(ui, debug_memory, device, avg_fps, fps, frame_time_ms, dt);
                        ui.separator();

                        draw_memory_section(ui, world);
                        ui.separator();

                        draw_scene_section(ui, world);
                    });
            });
        });

    debug_memory.scale_factor = new_scale_factor.clamp(0.5, 2.5);
}

// UI helper methods
fn draw_perf_section(
    ui: &mut egui::Ui,
    debug_memory: &mut DebugMemory,
    device: &wgpu::Device,
    avg_fps: f32,
    fps: f32,
    ft_ms: f32,
    dt: f32,
) {
    let mut new_profiler_enabled = debug_memory.profiler.enabled();

    ui.collapsing(
        egui::RichText::new("📊 Performance")
            .strong()
            .color(egui::Color32::LIGHT_BLUE),
        |ui| {
            ui.add_space(4.0);

            ui.columns(2, |cols| {
                let color = if avg_fps > 60.0 {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::LIGHT_RED
                };
                cols[0].vertical_centered(|ui| {
                    ui.label("Average FPS");
                    ui.heading(egui::RichText::new(format!("{:.0}", avg_fps)).color(color));
                    ui.small(format!("raw: {:.0}", fps));
                });
                cols[1].vertical_centered(|ui| {
                    ui.label("Frametime");
                    ui.heading(format!("{:.2} ms", ft_ms));
                    ui.small(format!("Δt: {:.4}s", dt));
                });
            });

            ui.add_space(8.0);

            // Compact Plot Styling Helper
            let create_plot = |ui: &mut egui::Ui,
                               id: &str,
                               label: &str,
                               data: &VecDeque<f32>,
                               color: egui::Color32| {
                ui.small(label);

                let reset_id = ui.id().with(id).with("reset");
                let mut reset_requested =
                    ui.data_mut(|d| d.get_temp::<bool>(reset_id).unwrap_or(false));

                const MAX_Y: f64 = 1200.0;

                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 6,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        let plot_response = Plot::new(id)
                            .view_aspect(4.0)
                            .include_y(0.0)
                            .include_y(MAX_Y)
                            .show_axes([false, true])
                            .label_formatter(|_, binary| format!("{:.1} FPS", binary.y))
                            .show(ui, |plot_ui| {
                                if reset_requested {
                                    plot_ui.set_auto_bounds([true, true]);
                                    reset_requested = false;
                                }

                                let points: PlotPoints = data
                                    .iter()
                                    .enumerate()
                                    .map(|(i, f)| [i as f64, *f as f64])
                                    .collect();

                                plot_ui.line(Line::new(label, points).color(color).width(1.5));
                            });

                        ui.data_mut(|d| d.insert_temp(reset_id, false));

                        let plot_rect = plot_response.response.rect;
                        let btn_size = egui::vec2(22.0, 22.0);
                        let btn_rect = egui::Rect::from_min_size(
                            plot_rect.left_bottom() + egui::vec2(8.0, -30.0),
                            btn_size,
                        );

                        ui.scope_builder(egui::UiBuilder::new().max_rect(btn_rect), |ui| {
                            let visuals = ui.visuals_mut();
                            visuals.widgets.inactive.weak_bg_fill =
                                egui::Color32::from_black_alpha(160);
                            visuals.widgets.hovered.weak_bg_fill =
                                egui::Color32::from_black_alpha(220);
                            if ui
                                .add(egui::Button::new(egui::RichText::new("⟲").size(14.0)))
                                .clicked()
                            {
                                ui.data_mut(|d| d.insert_temp(reset_id, true));
                                ui.ctx().request_repaint();
                            }
                        });
                    });
            };

            create_plot(
                ui,
                "rt_plot",
                "Real-Time",
                &debug_memory.fps_history,
                egui::Color32::from_rgb(100, 200, 255),
            );

            ui.add_space(20.0);

            create_plot(
                ui,
                "lt_plot",
                &format!("Session Trend ({}s interval)", FRAME_UPDATE_TIME),
                &debug_memory.fps_long_history,
                egui::Color32::LIGHT_GREEN,
            );

            ui.add_space(8.0);

            let cpu_spans = debug_memory.profiler.cpu_spans();
            if !cpu_spans.is_empty() && debug_memory.profiler.enabled() {
                debug_memory.cpu_history.push_back(cpu_spans);

                if debug_memory.cpu_history.len() > 100 {
                    debug_memory.cpu_history.pop_front();
                }
            }

            draw_timing_history(ui, "CPU Timings", &debug_memory.cpu_history, 100.0);

            let gpu_spans = debug_memory.profiler.gpu_spans(device);
            if !gpu_spans.is_empty() && debug_memory.profiler.enabled() {
                debug_memory.gpu_history.push_back(gpu_spans);

                if debug_memory.gpu_history.len() > 100 {
                    debug_memory.gpu_history.pop_front();
                }
            }

            ui.horizontal(|ui| ui.checkbox(&mut new_profiler_enabled, "Profiler"));

            ui.add_space(4.0);
        },
    );

    debug_memory.profiler.set_enabled(new_profiler_enabled);
}

fn draw_timing_history(
    ui: &mut egui::Ui,
    label: &str,
    history: &VecDeque<Vec<(String, u128, u128)>>,
    height: f32,
) {
    ui.label(label);

    if history.is_empty() {
        return;
    }

    let desired_size = egui::vec2(ui.available_width(), height);
    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
    let rect = response.rect.shrink2(egui::vec2(0.0, 0.0));
    let rect = egui::Rect::from_min_max(rect.min, rect.max - egui::vec2(6.0, 0.0));

    let frame_count = history.len();
    let column_width = rect.width() / frame_count as f32;
    let column_width = column_width.ceil();

    // newest frame on the right
    for (frame_idx, spans) in history.iter().enumerate() {
        let x0 = rect.left() + frame_idx as f32 * column_width;
        let x1 = rect.left() + (frame_idx as f32 + 1.0) * column_width;

        // total time per frame
        let total_ns = spans.iter().map(|(_, _, end)| *end).max().unwrap_or(1) as f32;

        let mut cursor_y = rect.bottom();

        for (name, start, end) in spans {
            let duration_ns = (*end - *start) as f32;
            let height_px = (duration_ns / total_ns) * rect.height();

            let y0 = cursor_y - height_px;
            let y1 = cursor_y;
            cursor_y = y0;

            let r = egui::Rect::from_min_max(
                egui::pos2(x0, y0),
                egui::pos2(x1.max(x0 + 1.0), y1.max(y0 + 1.0)),
            );

            let color = match name.as_str() {
                "instance_cull_pass" => egui::Color32::from_rgb(200, 100, 100),
                "cluster_cull_pass" => egui::Color32::from_rgb(200, 150, 100),
                "raster_pass" => egui::Color32::from_rgb(100, 200, 100),
                _ => egui::Color32::GRAY,
            };

            painter.rect_filled(r, 0.0, color);

            // tooltip per column
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                if r.contains(pos) {
                    let duration_us = duration_ns / 1000.0;

                    egui::Tooltip::for_widget(&response)
                        .at_pointer()
                        .show(|ui| {
                            ui.label(format!(
                                "Frame {}\n{}\n{:.2} µs",
                                frame_idx, name, duration_us
                            ));
                        });
                }
            }
        }
    }
}

fn draw_memory_section(ui: &mut egui::Ui, world: &World) {
    ui.collapsing(
        egui::RichText::new("💾 Memory")
            .strong()
            .color(egui::Color32::LIGHT_BLUE),
        |ui| {
            let (mesh_b, tex_b, mat_b) = world.assets().report_memory();
            let inst_h_b = world.instances().report_memory_host();
            let inst_d_b = world.instances().report_memory_device();
            let mlet_b = world.meshlets().report_memory();

            let to_mb = |b| b as f32 / (1024.0 * 1024.0);
            let to_kb = |b| b as f32 / 1024.0;

            ui.label("RAM (Scene & Assets)");
            egui::Grid::new("ram_grid")
                .num_columns(2)
                .spacing([60.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Instances:");
                    ui.monospace(format!("{:.2} MB", to_mb(inst_h_b)));
                    ui.end_row();
                    ui.label("Meshes:");
                    ui.monospace(format!("{:.2} MB", to_mb(mesh_b)));
                    ui.end_row();
                    ui.label("Materials:");
                    ui.monospace(format!("{:.1} KB", to_kb(mat_b)));
                    ui.end_row();
                    ui.strong("Total RAM:");
                    ui.strong(format!("{:.2} MB", to_mb(inst_h_b + mesh_b + mat_b)));
                    ui.end_row();
                });

            ui.add_space(8.0);

            ui.label("VRAM (GPU Buffers)");
            egui::Grid::new("vram_grid")
                .num_columns(2)
                .spacing([60.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Instances:");
                    ui.monospace(format!("{:.1} KB", to_kb(inst_d_b)));
                    ui.end_row();
                    ui.label("Meshlets:");
                    ui.monospace(format!("{:.2} MB", to_mb(mlet_b)));
                    ui.end_row();
                    ui.label("Textures:");
                    ui.monospace(format!("{:.2} MB", to_mb(tex_b)));
                    ui.end_row();
                    ui.strong("Total VRAM:");
                    ui.strong(format!("{:.2} MB", to_mb(inst_d_b + mlet_b + tex_b)));
                    ui.end_row();
                });

            ui.add_space(4.0);
        },
    );
}

fn draw_scene_section(ui: &mut egui::Ui, world: &World) {
    ui.collapsing(
        egui::RichText::new("🔍 Scene Explorer")
            .strong()
            .color(egui::Color32::LIGHT_BLUE),
        |ui| {
            ui.horizontal(|ui| {
                ui.small(format!(
                    "Instances: {}",
                    world.instances().scene_instance_count
                ));
                ui.separator();
                ui.small(format!("Meshlets: {}", world.instances().count_clusters()));
            });

            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_salt("scene_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    for (i, instance) in world.iter_instances().enumerate() {
                        draw_instance_info(ui, world, i, instance);
                    }
                });

            ui.add_space(4.0);

            ui.small(format!(
                "Pending Dependencies: {}",
                world.assets().pending_dependencies()
            ))
        },
    );
}

fn draw_instance_info(ui: &mut egui::Ui, world: &World, i: usize, instance: &mere_asset::Instance) {
    let name = world
        .get_meshlet_mesh(instance.meshlet)
        .map(|m| m.read().name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    ui.push_id(i, |ui| {
        ui.collapsing(format!("{}: {}", i, name), |ui| {
            ui.monospace(format!(
                "Transform:\n  Translation: {}\n  Rotation: {}\n  Scale: {}",
                instance.transform.translation,
                instance.transform.euler_angles(),
                instance.transform.scale,
            ));
            ui.monospace(format!("Clusters: {}", instance.meshlet_count));
        });
    });
}
