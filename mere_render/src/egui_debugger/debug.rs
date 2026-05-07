use crate::camera::CameraController;
use egui_plot::{Line, Plot, PlotPoints};
use mere_asset::World;
use mere_log::{Profiler, ResolvedSpan};
use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};
use winit::window::Window;

const FRAME_UPDATE_TIME: f32 = 0.8;

const BG: egui::Color32 = egui::Color32::from_rgb(14, 16, 20);
const PANEL: egui::Color32 = egui::Color32::from_rgb(18, 20, 24);
const CARD_FILL: egui::Color32 = egui::Color32::from_rgb(26, 28, 34);
const CARD_FILL_HOVER: egui::Color32 = egui::Color32::from_rgb(34, 37, 44);
const BORDER: egui::Color32 = egui::Color32::from_rgb(48, 52, 60);
const MUTED: egui::Color32 = egui::Color32::from_rgb(130, 135, 145);

const ACCENT: egui::Color32 = egui::Color32::from_rgb(120, 170, 255);
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(120, 220, 140);
const WARNING: egui::Color32 = egui::Color32::from_rgb(255, 190, 110);
const ERROR: egui::Color32 = egui::Color32::from_rgb(255, 120, 120);

const INSTANCE_ROW_HEIGHT: f32 = 28.0;
const INSTANCE_EXPANDED_HEIGHT: f32 = 120.0;

pub struct DebugMemory {
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
    pub fn new() -> Self {
        Self {
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
            let avg = self.accumulator / self.frame_count as f64;
            self.fps_long_history.push_back(avg as f32);
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
    profiler: &mut Profiler,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ctx: &egui::Context,
    window: &Window,
    world: &mut World,
    delta_time: Duration,
    lock_view: &mut bool,
) {
    apply_style(ctx);

    let delta_time_secs = delta_time.as_secs_f32();
    let fps = 1.0 / delta_time_secs.max(0.00001);
    let frame_time_ms = delta_time_secs * 1000.0;

    debug_memory.update_history(fps, delta_time_secs);

    let avg_fps = debug_memory.avg_fps().unwrap_or(fps);

    let mut scale_factor = debug_memory.scale_factor;

    let (width, height): (u32, u32) = window.inner_size().into();

    draw_overlay_controls(ctx, world, avg_fps, frame_time_ms, lock_view);

    egui::Window::new(
        egui::RichText::new("MeRe Engine Debugger")
            .monospace()
            .strong(),
    )
    .default_pos([10.0, 10.0])
    .default_width(360.0)
    .default_height(400.0)
    .min_width(350.0)
    .resizable(true)
    .frame(
        egui::Frame::window(&ctx.global_style())
            .fill(BG)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .corner_radius(12.0)
            .inner_margin(12.0),
    )
    .show(ctx, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);

        footer(
            ui,
            width,
            height,
            &mut scale_factor,
            debug_memory.scale_factor,
        );

        egui::ScrollArea::vertical()
            .id_salt("debug_main_scroll")
            .show(ui, |ui| {
                draw_perf_section(
                    ui,
                    debug_memory,
                    profiler,
                    device,
                    queue,
                    avg_fps,
                    fps,
                    frame_time_ms,
                    delta_time_secs,
                );

                draw_memory_section(ui, world);

                draw_scene_section(ui, debug_memory, world);

                ui.add_space(10.0);
            });
    });

    debug_memory.scale_factor = scale_factor.clamp(0.5, 2.5);
}

fn apply_style(ctx: &egui::Context) {
    ctx.global_style_mut(|style| {
        style.visuals.window_fill = BG;
        style.visuals.panel_fill = PANEL;
        style.visuals.extreme_bg_color = BG;
        style.visuals.widgets.noninteractive.bg_fill = CARD_FILL;
        style.visuals.widgets.inactive.bg_fill = CARD_FILL;
        style.visuals.widgets.hovered.bg_fill = CARD_FILL_HOVER;
        style.visuals.widgets.active.bg_fill = CARD_FILL_HOVER;
        style.visuals.widgets.open.bg_fill = CARD_FILL_HOVER;
        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::WHITE;
        style.visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
        style.visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
        style.visuals.selection.bg_fill = ACCENT.linear_multiply(0.4);
        style.visuals.collapsing_header_frame = false;
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(12);
        style.interaction.selectable_labels = false;

        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::monospace(14.0));

        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::monospace(11.0));

        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::monospace(12.0));

        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::monospace(8.0));
    });
}

fn draw_overlay_controls(
    ctx: &egui::Context,
    world: &mut World,
    avg_fps: f32,
    frame_time_ms: f32,
    lock_view: &mut bool,
) {
    egui::Area::new(egui::Id::new("overlay_controls"))
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .show(ctx, |ui| {
            card_frame()
                .fill(egui::Color32::from_black_alpha(210))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{:.0} FPS", avg_fps))
                                .color(SUCCESS)
                                .strong(),
                        );

                        ui.separator();

                        ui.label(
                            egui::RichText::new(format!("{:.2} ms", frame_time_ms)).color(ACCENT),
                        );
                    });

                    ui.separator();

                    ui.label(section_header("Main Camera"));

                    ui.checkbox(lock_view, "Lock Render View");

                    if ui.input(|i| i.key_pressed(egui::Key::L)) {
                        *lock_view = !*lock_view;
                    }

                    let camera = world.main_camera_mut();

                    let mut fov = camera.fov_y.to_degrees();

                    let response = ui.add(
                        egui::Slider::new(
                            &mut fov,
                            CameraController::MIN_ZOOM.to_degrees()
                                ..=CameraController::MAX_ZOOM.to_degrees(),
                        )
                        .text("FOV")
                        .fixed_decimals(1),
                    );

                    if response.changed() {
                        camera.fov_y = fov.to_radians();
                    }
                });
        });
}

fn draw_perf_section(
    ui: &mut egui::Ui,
    debug_memory: &mut DebugMemory,
    profiler: &mut Profiler,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    avg_fps: f32,
    fps: f32,
    frame_time_ms: f32,
    dt: f32,
) {
    section(ui, "Performance", |ui| {
        ui.columns(2, |cols| {
            stat_card(
                &mut cols[0],
                "Average FPS",
                format!("{:.0}", avg_fps),
                format!("raw {:.0}", fps),
                if avg_fps >= 60.0 {
                    SUCCESS
                } else if avg_fps >= 30.0 {
                    WARNING
                } else {
                    ERROR
                },
            );

            stat_card(
                &mut cols[1],
                "Frametime",
                format!("{:.2} ms", frame_time_ms),
                format!("Δt {:.4}", dt),
                ACCENT,
            );
        });

        egui::CollapsingHeader::new(section_header("FPS Graphs"))
            .default_open(true)
            .show(ui, |ui| {
                create_plot(
                    ui,
                    "rt_plot",
                    "Real-Time",
                    &debug_memory.fps_history,
                    ACCENT,
                );

                create_plot(
                    ui,
                    "lt_plot",
                    "Session Trend",
                    &debug_memory.fps_long_history,
                    SUCCESS,
                );
            });

        let mut profiler_enabled = profiler.enabled();

        egui::CollapsingHeader::new(section_header("Pass Timings")).show(ui, |ui| {
            let cpu_spans = profiler.cpu_spans();

            if profiler_enabled && !cpu_spans.is_empty() {
                debug_memory.cpu_history.push_back(cpu_spans);
                if debug_memory.cpu_history.len() > 100 {
                    debug_memory.cpu_history.pop_front();
                }
            }

            draw_timing_history(ui, "CPU Timings", &debug_memory.cpu_history, 120.0);

            let gpu_spans = profiler.gpu_spans(device, queue);

            if profiler_enabled && !gpu_spans.is_empty() {
                debug_memory.gpu_history.push_back(gpu_spans);
                if debug_memory.gpu_history.len() > 100 {
                    debug_memory.gpu_history.pop_front();
                }
            }

            draw_timing_history(ui, "GPU Timings", &debug_memory.gpu_history, 120.0);

            ui.add_space(6.0);

            ui.checkbox(&mut profiler_enabled, "Profiler");
        });

        profiler.set_enabled(profiler_enabled);
    });
}

fn draw_memory_section(ui: &mut egui::Ui, world: &World) {
    section(ui, "Memory", |ui| {
        let (mesh_b, tex_b, mat_b) = world.assets().report_memory();

        let inst_h_b = world.instances().report_memory_host();
        let inst_d_b = world.instances().report_memory_device();

        let mlet_b = world.meshlets().report_memory();

        let to_mb = |b| b as f32 / (1024.0 * 1024.0);
        let to_kb = |b| b as f32 / 1024.0;

        ui.columns(2, |cols| {
            card_frame().show(&mut cols[0], |ui| {
                ui.label(section_header("RAM"));

                memory_row(ui, "Instances", format!("{:.2} MB", to_mb(inst_h_b)));
                memory_row(ui, "Meshes", format!("{:.2} MB", to_mb(mesh_b)));
                memory_row(ui, "Materials", format!("{:.1} KB", to_kb(mat_b)));

                ui.separator();

                memory_row(
                    ui,
                    "Total",
                    format!("{:.2} MB", to_mb(inst_h_b + mesh_b + mat_b)),
                );
            });

            card_frame().show(&mut cols[1], |ui| {
                ui.label(section_header("VRAM"));

                memory_row(ui, "Instances", format!("{:.1} KB", to_kb(inst_d_b)));
                memory_row(ui, "Meshlets", format!("{:.2} MB", to_mb(mlet_b)));
                memory_row(ui, "Textures", format!("{:.2} MB", to_mb(tex_b)));

                ui.separator();

                memory_row(
                    ui,
                    "Total",
                    format!("{:.2} MB", to_mb(inst_d_b + mlet_b + tex_b)),
                );
            });
        });
    });
}

fn draw_scene_section(ui: &mut egui::Ui, debug_memory: &mut DebugMemory, world: &World) {
    section(ui, "Scene Explorer", |ui| {
        ui.horizontal_wrapped(|ui| {
            metric_chip(
                ui,
                format!("Instances {}", world.instances().scene_instance_count),
            );

            metric_chip(
                ui,
                format!("Clusters {}", world.instances().count_clusters()),
            );

            metric_chip(
                ui,
                format!("Pending Assets {}", world.assets().pending_dependencies()),
            );
        });

        card_frame().show(ui, |ui| {
            let instances: Vec<_> = world.instances().iter().collect();

            let open = &mut debug_memory.instance_open;

            egui::ScrollArea::vertical()
                .id_salt("instance_virtualized")
                .show_rows(ui, INSTANCE_ROW_HEIGHT, instances.len(), |ui, range| {
                    for index in range {
                        let instance = instances[index];

                        let is_open = open.contains(&index);

                        let row_height = if is_open {
                            INSTANCE_ROW_HEIGHT + INSTANCE_EXPANDED_HEIGHT
                        } else {
                            INSTANCE_ROW_HEIGHT
                        };

                        ui.allocate_ui(egui::vec2(ui.available_width(), row_height), |ui| {
                            let mesh_name = world
                                .get_meshlet_mesh(instance.meshlet_mesh)
                                .map(|m| m.read().name.clone())
                                .unwrap_or_else(|| "Unknown".to_string());

                            let response = ui.add_sized(
                                [ui.available_width(), INSTANCE_ROW_HEIGHT],
                                egui::Button::new(""),
                            );

                            ui.painter().text(
                                response.rect.left_center() + egui::vec2(10.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{:04}  {}", index, mesh_name),
                                egui::FontId::monospace(14.0),
                                egui::Color32::WHITE,
                            );

                            if response.clicked() {
                                if is_open {
                                    open.remove(&index);
                                } else {
                                    open.insert(index);
                                }
                            }

                            if is_open {
                                ui.add_space(6.0);

                                egui::Frame::new()
                                    .fill(PANEL)
                                    .stroke(egui::Stroke::new(1.0, BORDER))
                                    .corner_radius(8.0)
                                    .inner_margin(10.0)
                                    .show(ui, |ui| {
                                        info_line(
                                            ui,
                                            "Translation",
                                            format!("{}", instance.transform.translation),
                                        );

                                        info_line(
                                            ui,
                                            "Rotation",
                                            format!("{}", instance.transform.euler_angles()),
                                        );

                                        info_line(
                                            ui,
                                            "Scale",
                                            format!("{}", instance.transform.scale),
                                        );

                                        info_line(
                                            ui,
                                            "Meshlets",
                                            format!("{}", instance.meshlet_count),
                                        );
                                    });
                            }
                        });
                    }
                });
        });
    });
}

fn draw_timing_history(
    ui: &mut egui::Ui,
    label: &str,
    history: &VecDeque<Vec<(String, u128, u128)>>,
    height: f32,
) {
    if history.is_empty() {
        return;
    }

    ui.label(plot_title(label));

    let total_width = ui.available_width();

    card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            let graph_width = (total_width - 170.0).max(120.0);

            let (response, painter) =
                ui.allocate_painter(egui::vec2(graph_width, height), egui::Sense::hover());

            let rect = response.rect;

            painter.rect_filled(rect, 8.0, PANEL);

            let frame_count = history.len().max(1);
            let col_width = rect.width() / frame_count as f32;

            // clean spacing between frame columns
            let column_gap = 1.0 / ui.pixels_per_point();

            for (frame_idx, spans) in history.iter().enumerate() {
                let x0 = rect.left() + frame_idx as f32 * col_width + column_gap * 0.5;
                let x1 = rect.left() + (frame_idx + 1) as f32 * col_width - column_gap * 0.5;

                if frame_idx > 0 {
                    let line_x = x0 - column_gap * 0.5;

                    painter.line_segment(
                        [
                            egui::pos2(line_x, rect.top() + 4.0),
                            egui::pos2(line_x, rect.bottom() - 4.0),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(10)),
                    );
                }

                let total_ns = spans.iter().map(|(_, _, end)| *end).max().unwrap_or(1) as f32;

                let mut cursor_y = rect.bottom();

                for (name, start, end) in spans.iter().rev() {
                    let duration_ns = (*end - *start) as f32;

                    let h =
                        ((duration_ns / total_ns) * rect.height()).max(6.0 / ui.pixels_per_point());

                    let y0 = cursor_y - h;
                    let y1 = cursor_y;

                    cursor_y = y0;

                    let bar_rect = egui::Rect::from_min_max(
                        egui::pos2(x0, y0 + 1.0),
                        egui::pos2(x1, y1 - 1.0),
                    );

                    let color = pass_color(name);

                    // rounded bars
                    painter.rect_filled(bar_rect, egui::CornerRadius::same(3), color);

                    if response.hovered() {
                        if let Some(pos) = ui.ctx().pointer_hover_pos() {
                            if bar_rect.expand(0.5).contains(pos) {
                                response.clone().on_hover_ui(|ui| {
                                    ui.label(egui::RichText::new(name).strong().color(color));

                                    ui.separator();

                                    info_line(ui, "Frame", format!("{frame_idx}"));

                                    info_line(
                                        ui,
                                        "Start",
                                        format!("{:.2} µs", *start as f32 / 1000.0),
                                    );

                                    info_line(ui, "End", format!("{:.2} µs", *end as f32 / 1000.0));

                                    info_line(ui, "Duration", format_duration_ns(duration_ns));
                                });
                            }
                        }
                    }
                }
            }

            // outer border
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(8),
                egui::Stroke::new(1.0, BORDER),
                egui::StrokeKind::Outside,
            );

            ui.vertical(|ui| {
                ui.global_style_mut(|style| {
                    style.spacing.item_spacing = egui::vec2(style.spacing.item_spacing.x, 0.0)
                });

                let latest = history.back().unwrap();

                for (name, start, end) in latest.iter() {
                    let duration_ns = (*end - *start) as f32;

                    let color = pass_color(name);

                    egui::Frame::new()
                        .fill(PANEL)
                        .stroke(egui::Stroke::new(1.0, BORDER))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .outer_margin(0.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(color, egui::RichText::new("■").size(8.0));

                                ui.label(egui::RichText::new(name).size(8.0).color(color).strong());
                            });
                        })
                        .response
                        .on_hover_ui(|ui| {
                            info_line(ui, "Start", format!("{:.2} µs", *start as f32 / 1000.0));

                            info_line(ui, "End", format!("{:.2} µs", *end as f32 / 1000.0));

                            info_line(ui, "Duration", format_duration_ns(duration_ns));
                        });
                }
            });
        });
    });
}

fn create_plot(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    data: &VecDeque<f32>,
    color: egui::Color32,
) {
    let reset_id = ui.id().with(id).with("reset");
    let mut reset_requested = ui.data_mut(|d| d.get_temp::<bool>(reset_id).unwrap_or(false));

    ui.label(plot_title(label));

    card_frame().show(ui, |ui| {
        let points: PlotPoints = data
            .iter()
            .enumerate()
            .map(|(i, v)| [i as f64, *v as f64])
            .collect();

        let plot_rect = Plot::new(id)
            .height(140.0)
            .allow_scroll(false)
            .show_axes([false, true])
            .include_y(0.0)
            .label_formatter(|_, p| format!("{:.1} FPS", p.y))
            .show(ui, |plot_ui| {
                if reset_requested {
                    plot_ui.set_auto_bounds([true, true]);
                    reset_requested = false;
                }

                plot_ui.line(Line::new(label, points).color(color).width(2.0));
            })
            .response
            .rect;

        ui.data_mut(|d| d.insert_temp(reset_id, false));

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

fn footer(ui: &mut egui::Ui, width: u32, height: u32, scale_factor: &mut f32, current_scale: f32) {
    egui::Panel::bottom("footer").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(muted(format!("Resolution {}×{}", width, height)));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("+").clicked() {
                    *scale_factor += 0.1;
                }

                ui.label(egui::RichText::new(format!("{:.1}", current_scale)).monospace());

                if ui.small_button("-").clicked() {
                    *scale_factor -= 0.1;
                }

                ui.label(muted("UI Scale"));
            });
        });
    });
}

fn section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(section_header(title)).show(ui, |ui| {
        ui.add_space(4.0);
        add_contents(ui);
    });
}

fn stat_card(ui: &mut egui::Ui, title: &str, value: String, sub: String, color: egui::Color32) {
    card_frame().show(ui, |ui| {
        ui.label(muted(title));

        ui.label(egui::RichText::new(value).size(18.0).strong().color(color));

        ui.label(muted(sub));
    });
}

fn memory_row(ui: &mut egui::Ui, name: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(muted(name));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).monospace().strong());
        });
    });
}

fn info_line(ui: &mut egui::Ui, key: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).color(MUTED).monospace());

        ui.label(
            egui::RichText::new(value)
                .monospace()
                .color(egui::Color32::WHITE),
        );
    });
}

fn metric_chip(ui: &mut egui::Ui, text: impl Into<String>) {
    let text = text.into();

    let galley = ui
        .painter()
        .layout_no_wrap(text.clone(), egui::FontId::monospace(13.0), MUTED);

    let padding = egui::vec2(20.0, 10.0);

    let desired_size = galley.size() + padding;

    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(u8::MAX),
        PANEL,
        egui::Stroke::new(1.0, BORDER),
        egui::StrokeKind::Outside,
    );

    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, MUTED);
}

fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD_FILL)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(10.0)
        .inner_margin(10.0)
}

fn section_header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .strong()
        .size(15.0)
        .color(ACCENT)
}

fn plot_title(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .size(13.0)
        .color(MUTED)
}

fn muted(text: impl ToString) -> egui::RichText {
    egui::RichText::new(text.to_string())
        .monospace()
        .color(MUTED)
}

fn pass_color(name: &str) -> egui::Color32 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    let hash = hasher.finish();

    let hue = (hash % 360) as f32 / 360.0;

    egui::ecolor::Hsva::new(hue, 0.7, 0.9, 1.0).into()
}

fn format_duration_ns(duration_ns: f32) -> String {
    if duration_ns < 1_000.0 {
        "< 1.00 µs".to_string()
    } else if duration_ns < 1_000_000.0 {
        format!("{:.02} µs", duration_ns / 1_000.0)
    } else {
        format!("{:.02} ms", duration_ns / 1_000_000.0)
    }
}
