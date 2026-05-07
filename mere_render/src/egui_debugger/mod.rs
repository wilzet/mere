use debug::DebugMemory;
use egui::Context;
use egui_wgpu::Renderer;
use egui_winit::State;
use mere_asset::World;
use winit::event::WindowEvent;
use winit::window::Window;

mod debug;
mod profiler;

pub use profiler::Profiler;

pub struct EguiRenderer {
    state: State,
    renderer: Renderer,
    frame_started: bool,
    debug_memory: DebugMemory,
}

impl EguiRenderer {
    pub fn new(
        device: &wgpu::Device,
        output_color_format: wgpu::TextureFormat,
        output_depth_format: Option<wgpu::TextureFormat>,
        msaa_samples: u32,
        window: &Window,
    ) -> EguiRenderer {
        let egui_context = Context::default();

        let egui_state = egui_winit::State::new(
            egui_context,
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(2 * 1024), // default dimension is 2048
        );
        let egui_renderer = Renderer::new(
            device.into(),
            output_color_format.into(),
            egui_wgpu::RendererOptions {
                msaa_samples,
                depth_stencil_format: output_depth_format.into(),
                dithering: true,
                predictable_texture_filtering: false,
            },
        );

        EguiRenderer {
            state: egui_state,
            renderer: egui_renderer,
            frame_started: false,
            debug_memory: DebugMemory::new(),
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);
        let ctx = self.state.egui_ctx();

        if response.consumed || ctx.is_pointer_over_egui() {
            return true;
        }

        ctx.pointer_interact_pos().map_or(false, |pos| {
            let style = ctx.global_style();
            let margin = style
                .interaction
                .resize_grab_radius_corner
                .max(style.interaction.resize_grab_radius_side);

            ctx.memory(|mem| {
                mem.areas().visible_layer_ids().iter().any(|layer_id| {
                    if layer_id.order == egui::Order::Background {
                        return false;
                    }

                    mem.area_rect(layer_id.id)
                        .map_or(false, |rect| rect.expand(margin).contains(pos))
                })
            })
        })
    }

    pub fn begin_frame(&mut self, window: &Window) {
        let raw_input = self.state.take_egui_input(window);
        self.state.egui_ctx().begin_pass(raw_input);
        self.frame_started = true;
    }

    pub fn debug_window(
        &mut self,
        profiler: &mut Profiler,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        window: &Window,
        world: &mut World,
        delta_time: std::time::Duration,
        lock_view: &mut bool,
    ) {
        debug::debugger(
            &mut self.debug_memory,
            profiler,
            device,
            queue,
            &self.state.egui_ctx(),
            window,
            world,
            delta_time,
            lock_view,
        );
    }

    pub fn end_frame_and_draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &Window,
        target_view: &wgpu::TextureView,
    ) {
        assert!(
            self.frame_started,
            "begin_frame must be called before end_frame_and_draw can be called!"
        );

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [
                target_view.texture().width(),
                target_view.texture().height(),
            ],
            pixels_per_point: window.scale_factor() as f32 * self.debug_memory.scale_factor(),
        };

        self.state
            .egui_ctx()
            .set_pixels_per_point(screen_descriptor.pixels_per_point);

        let output = self.state.egui_ctx().end_pass();

        self.state
            .handle_platform_output(window, output.platform_output);

        let tris = self
            .state
            .egui_ctx()
            .tessellate(output.shapes, output.pixels_per_point);

        self.upload_textures(device, queue, output.textures_delta);
        self.draw(
            device,
            queue,
            encoder,
            target_view,
            &screen_descriptor,
            &tris,
        );

        self.frame_started = false;
    }

    fn upload_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: egui::TexturesDelta,
    ) {
        for (id, delta) in &textures.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }

        for x in &textures.free {
            self.renderer.free_texture(x)
        }
    }

    fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        tris: &[egui::ClippedPrimitive],
    ) {
        self.renderer
            .update_buffers(device, queue, encoder, tris, screen_descriptor);

        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            label: Some("egui main render pass"),
            occlusion_query_set: None,
            multiview_mask: None,
        });

        self.renderer
            .render(&mut pass.forget_lifetime(), tris, screen_descriptor);
    }
}
