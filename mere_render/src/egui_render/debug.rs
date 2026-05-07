use super::profiler::{Profiler, ResolvedSpan};
use crate::camera::CameraController;
use egui_plot::{Line, Plot, PlotPoints};
use mere_asset::World;
use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};
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
    instance_open: HashSet<usize>,
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
            instance_open: HashSet::new(),
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

const CARD_FILL: egui::Color32 = egui::Color32::from_rgb(28, 30, 36);
const BORDER: egui::Color32 = egui::Color32::from_rgb(48, 52, 60);
const MUTED: egui::Color32 = egui::Color32::from_rgb(130, 135, 145);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(120, 170, 255);
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(120, 220, 140);
const WARNING: egui::Color32 = egui::Color32::from_rgb(255, 190, 110);
const ERROR: egui::Color32 = egui::Color32::from_rgb(255, 120, 120);

pub fn debugger(
    debug_memory: &mut DebugMemory,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ctx: &egui::Context,
    window: &Window,
    world: &mut World,
    delta_time: Duration,
    lock_view: &mut bool,
) {
    ctx.global_style_mut(|style| {
        style.visuals.window_fill = egui::Color32::from_rgb(16, 18, 22);
        style.visuals.panel_fill = egui::Color32::from_rgb(20, 22, 26);
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(10, 12, 16);
        style.visuals.widgets.noninteractive.bg_fill = CARD_FILL;
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(32, 35, 42);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(42, 46, 56);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(52, 58, 72);
        style.visuals.widgets.open.bg_fill = egui::Color32::from_rgb(38, 42, 50);
        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(210, 210, 210);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(70, 100, 160);
        style.visuals.collapsing_header_frame = true;
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(10);
    });

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
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_black_alpha(190))
                .stroke(egui::Stroke::new(1.0, BORDER))
                .corner_radius(8.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;

                        ui.label(
                            egui::RichText::new(format!("{:.0} FPS", avg_fps))
                                .monospace()
                                .strong()
                                .size(18.0)
                                .color(egui::Color32::LIGHT_GREEN),
                        );

                        ui.separator();

                        ui.label(
                            egui::RichText::new(format!("{:.2} ms", frame_time_ms))
                                .monospace()
                                .color(egui::Color32::LIGHT_BLUE),
                        );
                    });

                    ui.separator();

                    ui.label(
                        egui::RichText::new("Main Camera")
                            .monospace()
                            .color(egui::Color32::LIGHT_GRAY),
                    );

                    ui.checkbox(
                        lock_view,
                        egui::RichText::new("Lock Render View")
                            .monospace()
                            .color(egui::Color32::LIGHT_GRAY),
                    );
                    if ui.input(|i| i.key_pressed(egui::Key::L)) {
                        *lock_view = !*lock_view;
                    }

                    let main_camera = world.main_camera_mut();
                    let mut fov_y = main_camera.fov_y.to_degrees();

                    let response = ui.add(
                        egui::Slider::new(
                            &mut fov_y,
                            CameraController::MIN_ZOOM.to_degrees()
                                ..=CameraController::MAX_ZOOM.to_degrees(),
                        )
                        .fixed_decimals(1)
                        .text(
                            egui::RichText::new("FOV")
                                .monospace()
                                .color(egui::Color32::LIGHT_GRAY),
                        ),
                    );

                    if response.changed() {
                        main_camera.fov_y = fov_y.to_radians();
                    }
                });
        });

    egui::Window::new("MeRe Engine Debugger")
        .resizable(true)
        .vscroll(false)
        .hscroll(false)
        .default_width(320.0)
        .default_height(400.0)
        .anchor(egui::Align2::LEFT_TOP, [10.0, 10.0])
        .frame(
            egui::Frame::window(&ctx.global_style())
                .fill(egui::Color32::from_rgb(16, 18, 22))
                .stroke(egui::Stroke::new(1.0, BORDER))
                .corner_radius(12.0),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;

            // FOOTER
            egui::Panel::bottom("debug_footer").show_inside(ui, |ui| {
                ui.add_space(2.0);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Res: {}x{}", width, height))
                            .monospace()
                            .color(MUTED),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("+").clicked() {
                            new_scale_factor += 0.1;
                        }

                        ui.label(
                            egui::RichText::new(format!("{:.1}", debug_memory.scale_factor))
                                .monospace(),
                        );

                        if ui.small_button("-").clicked() {
                            new_scale_factor -= 0.1;
                        }

                        ui.label("UI Scale");
                    });
                });
            });

            // MAIN CONTENT
            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("main_debug_scroll")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        draw_perf_section(
                            ui,
                            debug_memory,
                            device,
                            queue,
                            avg_fps,
                            fps,
                            frame_time_ms,
                            dt,
                        );

                        ui.add_space(8.0);

                        draw_memory_section(ui, world);

                        ui.add_space(8.0);

                        draw_scene_section(ui, debug_memory, world);
                    });
            });
        });

    debug_memory.scale_factor = new_scale_factor.clamp(0.5, 2.5);
}

fn pass_color(name: &str) -> egui::Color32 {
    match name {
        "instance_cull_pass" => egui::Color32::from_rgb(220, 120, 120),
        "cluster_cull_pass" => egui::Color32::from_rgb(240, 180, 120),
        "raster_pass" => egui::Color32::from_rgb(120, 220, 140),
        "lighting_pass" => egui::Color32::from_rgb(120, 180, 255),
        _ => egui::Color32::from_gray(110),
    }
}

fn section_header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .strong()
        .size(16.0)
        .color(ACCENT)
}

fn muted(text: impl ToString) -> egui::RichText {
    egui::RichText::new(text.to_string())
        .monospace()
        .color(MUTED)
}

fn card_frame() -> egui::Frame {
    egui::Frame::group(&egui::Style::default())
        .fill(CARD_FILL)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(8.0)
        .inner_margin(10.0)
}

fn draw_perf_section(
    ui: &mut egui::Ui,
    debug_memory: &mut DebugMemory,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    avg_fps: f32,
    fps: f32,
    ft_ms: f32,
    dt: f32,
) {
    let mut profiler_enabled = debug_memory.profiler.enabled();

    egui::CollapsingHeader::new(section_header("Performance")).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;

        ui.horizontal_top(|ui| {
            let fps_color = if avg_fps >= 60.0 {
                egui::Color32::LIGHT_GREEN
            } else {
                egui::Color32::LIGHT_RED
            };

            ui.set_width(ui.available_width() - 20.0);
            let card_w = (ui.available_width() - 8.0) * 0.5;

            ui.vertical(|ui| {
                ui.set_width(card_w);
                card_frame().show(ui, |ui| {
                    ui.label(egui::RichText::new("Average FPS").small().color(MUTED));
                    ui.heading(egui::RichText::new(format!("{:.0}", avg_fps)).color(fps_color));
                    ui.small(format!("raw {:.0}", fps));
                });
            });

            ui.add_space(8.0);

            ui.vertical(|ui| {
                ui.set_width(card_w);
                card_frame().show(ui, |ui| {
                    ui.label(egui::RichText::new("Frametime").small().color(MUTED));
                    ui.heading(format!("{:.2} ms", ft_ms));
                    ui.small(format!("Δt {:.4}", dt));
                });
            });
        });

        ui.add_space(10.0);

        create_plot(
            ui,
            "rt_plot",
            "Real-Time FPS",
            &debug_memory.fps_history,
            egui::Color32::from_rgb(100, 200, 255),
        );

        ui.add_space(10.0);

        create_plot(
            ui,
            "lt_plot",
            &format!("Session Trend ({}s)", FRAME_UPDATE_TIME),
            &debug_memory.fps_long_history,
            egui::Color32::LIGHT_GREEN,
        );

        ui.add_space(12.0);

        let cpu_spans = debug_memory.profiler.cpu_spans();

        if !cpu_spans.is_empty() && profiler_enabled {
            debug_memory.cpu_history.push_back(cpu_spans);

            if debug_memory.cpu_history.len() > 100 {
                debug_memory.cpu_history.pop_front();
            }
        }

        draw_timing_history(ui, "CPU Timings", &debug_memory.cpu_history, 100.0);

        ui.add_space(10.0);

        let gpu_spans = debug_memory.profiler.gpu_spans(device, queue);

        if !gpu_spans.is_empty() && profiler_enabled {
            debug_memory.gpu_history.push_back(gpu_spans);

            if debug_memory.gpu_history.len() > 100 {
                debug_memory.gpu_history.pop_front();
            }
        }

        draw_timing_history(ui, "GPU Timings", &debug_memory.gpu_history, 100.0);

        ui.add_space(10.0);

        ui.checkbox(&mut profiler_enabled, "Profiler");
    });

    debug_memory.profiler.set_enabled(profiler_enabled);
}

fn create_plot(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    data: &VecDeque<f32>,
    color: egui::Color32,
) {
    ui.label(egui::RichText::new(label).small().color(MUTED));

    let reset_id = ui.id().with(id).with("reset");
    let mut reset_requested = ui.data_mut(|d| d.get_temp::<bool>(reset_id).unwrap_or(false));

    card_frame().show(ui, |ui| {
        let plot_response = Plot::new(id)
            .height(120.0)
            .allow_scroll(false)
            .include_y(0.0)
            .show_axes([false, true])
            .label_formatter(|_, p| format!("{:.1} FPS", p.y))
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

                plot_ui.line(Line::new(label, points).color(color).width(2.0));
            });

        ui.data_mut(|d| d.insert_temp(reset_id, false));

        let plot_rect = plot_response.response.rect;

        let btn_rect = egui::Rect::from_min_size(
            plot_rect.left_bottom() + egui::vec2(8.0, -30.0),
            egui::vec2(22.0, 22.0),
        );

        ui.scope_builder(egui::UiBuilder::new().max_rect(btn_rect), |ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("⟲").size(13.0))
                        .fill(egui::Color32::from_black_alpha(180)),
                )
                .clicked()
            {
                ui.data_mut(|d| d.insert_temp(reset_id, true));
                ui.ctx().request_repaint();
            }
        });
    });
}

fn draw_timing_history(
    ui: &mut egui::Ui,
    label: &str,
    history: &VecDeque<Vec<(String, u128, u128)>>,
    height: f32,
) {
    ui.label(egui::RichText::new(label).size(14.0).strong());

    ui.add_space(6.0);

    if history.is_empty() {
        return;
    }

    card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            let side_width = 160.0;
            let graph_width = (ui.available_width() - side_width).max(100.0);

            let desired_size = egui::vec2(graph_width, height);

            let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
            let rect = response.rect;

            painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(24, 26, 30));

            let frame_count = history.len();
            let column_width = rect.width() / frame_count as f32;

            let pixel = ui.ctx().pixels_per_point();
            let gutter = 1.0 / pixel;

            for (frame_idx, spans) in history.iter().enumerate() {
                let x0 = rect.left() + frame_idx as f32 * column_width;

                let x1 = x0 + column_width - gutter;

                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x0, rect.top()),
                        egui::pos2(x1, rect.bottom()),
                    ),
                    0.0,
                    egui::Color32::from_black_alpha(8),
                );

                let total_ns = spans.iter().map(|(_, _, end)| *end).max().unwrap_or(1) as f32;

                let mut cursor_y = rect.bottom();

                for (name, start, end) in spans {
                    let duration_ns = (*end - *start) as f32;

                    let height_px = (duration_ns / total_ns) * rect.height();

                    let y0 = cursor_y - height_px;
                    let y1 = cursor_y;

                    cursor_y = y0;

                    let r = egui::Rect::from_min_max(
                        egui::pos2(x0, y0 + gutter),
                        egui::pos2(x1, y1 - gutter),
                    );

                    let color = pass_color(name);

                    painter.rect_filled(r, 0.0, color);

                    // hover tooltip unchanged
                    if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                        if r.contains(pos) {
                            egui::Tooltip::for_widget(&response)
                                .at_pointer()
                                .show(|ui| {
                                    ui.set_min_width(180.0);
                                    ui.label(egui::RichText::new(name).strong().color(color));
                                    ui.separator();
                                    egui::Grid::new(egui::Id::new(format!("{label}_{name}")))
                                        .num_columns(2)
                                        .spacing([12.0, 4.0])
                                        .show(ui, |ui| {
                                            ui.label("Frame");
                                            ui.monospace(format!("{frame_idx}"));
                                            ui.end_row();

                                            ui.label("Start");
                                            ui.monospace(format!(
                                                "{:.2} µs",
                                                *start as f32 / 1000.0
                                            ));
                                            ui.end_row();

                                            ui.label("End");
                                            ui.monospace(format!("{:.2} µs", *end as f32 / 1000.0));
                                            ui.end_row();

                                            ui.label("Duration");
                                            ui.monospace(format!("{:.2} µs", duration_ns / 1000.0));
                                            ui.end_row();
                                        });
                                });
                        }
                    }
                }
            }

            painter.rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.0, egui::Color32::from_black_alpha(40)),
                egui::StrokeKind::Outside,
            );

            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    let latest_frame = history.back().unwrap();

                    for (name, start, end) in latest_frame.iter().rev() {
                        let duration_ns = (*end - *start) as f32;
                        let color = pass_color(name);

                        egui::Frame::new()
                            .fill(egui::Color32::from_white_alpha(6))
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::same(6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(color, egui::RichText::new("■").strong());

                                    ui.label(egui::RichText::new(name).strong().color(color));
                                });
                            })
                            .response
                            .on_hover_ui(|ui| {
                                ui.set_min_width(180.0);

                                ui.label(egui::RichText::new(name).strong().color(color));

                                ui.separator();

                                egui::Grid::new(egui::Id::new(format!("{label}_{name}")))
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        ui.label("Start");
                                        ui.monospace(format!("{:.2} µs", *start as f32 / 1000.0));
                                        ui.end_row();

                                        ui.label("End");
                                        ui.monospace(format!("{:.2} µs", *end as f32 / 1000.0));
                                        ui.end_row();

                                        ui.label("Duration");
                                        ui.monospace(format!("{:.2} µs", duration_ns / 1000.0));
                                        ui.end_row();
                                    });
                            });
                    }
                },
            );
        });
    });
}

fn draw_memory_section(ui: &mut egui::Ui, world: &World) {
    egui::CollapsingHeader::new(section_header("Memory")).show(ui, |ui| {
        let (mesh_b, tex_b, mat_b) = world.assets().report_memory();
        let inst_h_b = world.instances().report_memory_host();
        let inst_d_b = world.instances().report_memory_device();
        let mlet_b = world.meshlets().report_memory();

        let to_mb = |b| b as f32 / (1024.0 * 1024.0);
        let to_kb = |b| b as f32 / 1024.0;

        ui.horizontal_top(|ui| {
            ui.set_width(ui.available_width());
            let card_w = (ui.available_width() - 8.0) * 0.5;

            ui.allocate_ui(egui::vec2(card_w, 0.0), |ui| {
                ui.label(egui::RichText::new("RAM").strong());

                ui.add_space(6.0);

                egui::Grid::new("ram_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Instances");
                        ui.monospace(format!("{:.2} MB", to_mb(inst_h_b)));
                        ui.end_row();

                        ui.label("Meshes");
                        ui.monospace(format!("{:.2} MB", to_mb(mesh_b)));
                        ui.end_row();

                        ui.label("Materials");
                        ui.monospace(format!("{:.1} KB", to_kb(mat_b)));
                        ui.end_row();

                        ui.separator();
                        ui.separator();
                        ui.end_row();

                        ui.strong("Total");
                        ui.strong(format!("{:.2} MB", to_mb(inst_h_b + mesh_b + mat_b)));
                        ui.end_row();
                    });
            });

            ui.allocate_ui(egui::vec2(card_w, 0.0), |ui| {
                ui.label(egui::RichText::new("VRAM").strong());

                ui.add_space(6.0);

                egui::Grid::new("vram_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Instances");
                        ui.monospace(format!("{:.1} KB", to_kb(inst_d_b)));
                        ui.end_row();

                        ui.label("Meshlets");
                        ui.monospace(format!("{:.2} MB", to_mb(mlet_b)));
                        ui.end_row();

                        ui.label("Textures");
                        ui.monospace(format!("{:.2} MB", to_mb(tex_b)));
                        ui.end_row();

                        ui.separator();
                        ui.separator();
                        ui.end_row();

                        ui.strong("Total");
                        ui.strong(format!("{:.2} MB", to_mb(inst_d_b + mlet_b + tex_b)));
                        ui.end_row();
                    });
            });
        });
    });
}

fn draw_scene_section(ui: &mut egui::Ui, debug_memory: &mut DebugMemory, world: &World) {
    egui::CollapsingHeader::new(section_header("Scene Explorer")).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            metric_chip(
                ui,
                format!("Instances: {}", world.instances().scene_instance_count),
            );

            metric_chip(
                ui,
                format!("Clusters: {}", world.instances().count_clusters()),
            );

            metric_chip(
                ui,
                format!("Pending: {}", world.assets().pending_dependencies()),
            );
        });

        ui.add_space(10.0);

        card_frame().show(ui, |ui| {
            let instances = world.instances().iter().collect::<Vec<_>>();

            const ROW_HEIGHT: f32 = 20.0;
            const EXPANDED_HEIGHT: f32 = 250.0;

            let open = &mut debug_memory.instance_open;

            egui::ScrollArea::vertical()
                .id_salt("instance_virtual_scroll")
                .min_scrolled_height(200.0)
                .auto_shrink([false, false])
                .show_rows(
                    ui,
                    ROW_HEIGHT + EXPANDED_HEIGHT,
                    instances.len(),
                    |ui, range| {
                        for i in range {
                            let instance = &instances[i];

                            let is_open = open.contains(&i);

                            let total_height = if is_open {
                                ROW_HEIGHT + EXPANDED_HEIGHT
                            } else {
                                ROW_HEIGHT
                            };

                            ui.allocate_ui(egui::vec2(ui.available_width(), total_height), |ui| {
                                let name = world
                                    .get_meshlet_mesh(instance.meshlet)
                                    .map(|m| m.read().name.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());

                                let button_text = format!("{:04}  {}", i, name);

                                let response = ui.add_sized(
                                    [ui.available_width(), ROW_HEIGHT],
                                    egui::Button::new(egui::RichText::new(button_text).monospace()),
                                );

                                if response.clicked() {
                                    if is_open {
                                        open.remove(&i);
                                    } else {
                                        open.insert(i);
                                    }
                                }

                                if is_open {
                                    ui.add_space(4.0);

                                    card_frame().fill(egui::Color32::from_rgb(24, 26, 30)).show(
                                        ui,
                                        |ui| {
                                            ui.label(muted(format!(
                                                "Translation: {}",
                                                instance.transform.translation
                                            )));

                                            ui.label(muted(format!(
                                                "Rotation: {}",
                                                instance.transform.euler_angles()
                                            )));

                                            ui.label(muted(format!(
                                                "Scale: {}",
                                                instance.transform.scale
                                            )));

                                            ui.add_space(4.0);

                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Meshlets: {}",
                                                    instance.meshlet_count
                                                ))
                                                .monospace()
                                                .color(ACCENT),
                                            );
                                        },
                                    );
                                }
                            });
                        }
                    },
                );
        });
    });
}

fn metric_chip(ui: &mut egui::Ui, text: String) {
    card_frame()
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(muted(text));
        });
}
