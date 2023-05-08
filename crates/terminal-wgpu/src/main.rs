use wgpu::CompositeAlphaMode;
use wgpu_glyph::{ab_glyph, GlyphBrushBuilder, Section, Text};
use winit::event_loop::EventLoop;

struct WgpuRenderer {
    glyph_brush: wgpu_glyph::GlyphBrush<()>,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    staging_belt: wgpu::util::StagingBelt,
    size: winit::dpi::PhysicalSize<u32>,
    render_format: wgpu::TextureFormat,
}

impl WgpuRenderer {
    pub fn new(event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
        let window = winit::window::WindowBuilder::new()
            .with_resizable(true)
            .build(event_loop)
            .unwrap();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = unsafe { instance.create_surface(&window)? };

        // Initialize GPU
        let (device, queue) = futures::executor::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Request adapter");

            adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Request device")
        });

        // Create staging belt
        let staging_belt = wgpu::util::StagingBelt::new(1024);

        // Prepare swap chain
        // TODO: get available render_format
        let render_format = wgpu::TextureFormat::Bgra8Unorm;
        let size = window.inner_size();

        // Prepare glyph_brush
        let inconsolata =
            ab_glyph::FontArc::try_from_slice(include_bytes!("Inconsolata-Regular.ttf"))?;

        let glyph_brush = GlyphBrushBuilder::using_font(inconsolata).build(&device, render_format);

        window.request_redraw();

        let mut this = Self {
            glyph_brush,
            surface,
            device,
            queue,
            staging_belt,
            size,
            render_format,
        };
        this.configure_surface();
        Ok(this)
    }

    fn configure_surface(&mut self) {
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.render_format,
                width: self.size.width,
                height: self.size.height,
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
        );
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.configure_surface();
    }

    fn redraw(&mut self) {
        // Get a command encoder for the current frame
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Redraw"),
            });

        // Get the next frame
        let frame = self.surface.get_current_texture().expect("Get next frame");
        let view = &frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Clear frame
        {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.4,
                            g: 0.4,
                            b: 0.4,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }

        self.glyph_brush.queue(Section {
            screen_position: (30.0, 30.0),
            bounds: (self.size.width as f32, self.size.height as f32),
            text: vec![Text::new("Hello wgpu_glyph 𑴭!")
                .with_color([0.0, 0.0, 0.0, 1.0])
                .with_scale(40.0)],
            ..Section::default()
        });

        self.glyph_brush.queue(Section {
            screen_position: (30.0, 90.0),
            bounds: (self.size.width as f32, self.size.height as f32),
            text: vec![Text::new("Hello wgpu_glyph!")
                .with_color([1.0, 1.0, 1.0, 1.0])
                .with_scale(40.0)],
            ..Section::default()
        });

        // Draw the text!
        self.glyph_brush
            .draw_queued(
                &self.device,
                &mut self.staging_belt,
                &mut encoder,
                view,
                self.size.width,
                self.size.height,
            )
            .expect("Draw queued");

        // Submit the work!
        self.staging_belt.finish();
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        // Recall unused staging buffers
        self.staging_belt.recall();
    }
}

struct Terminal {
    renderer: WgpuRenderer,
}

impl Terminal {
    fn new(event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
        Ok(Self {
            renderer: WgpuRenderer::new(event_loop)?,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.renderer.resize(new_size)
    }

    fn redraw(&mut self) {
        self.renderer.redraw()
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    // Open window and create a surface
    let event_loop = winit::event_loop::EventLoop::new();
    let mut term = Terminal::new(&event_loop)?;

    event_loop.run(move |event, _, control_flow| match event {
        winit::event::Event::WindowEvent {
            event: winit::event::WindowEvent::CloseRequested,
            ..
        } => *control_flow = winit::event_loop::ControlFlow::Exit,
        winit::event::Event::WindowEvent {
            event: winit::event::WindowEvent::Resized(new_size),
            ..
        } => term.resize(new_size),
        winit::event::Event::RedrawRequested { .. } => {
            term.redraw();
        }
        _ => {
            *control_flow = winit::event_loop::ControlFlow::Wait;
        }
    })
}
