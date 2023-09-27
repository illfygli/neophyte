use super::{
    depth_texture::DepthTexture,
    grids::Grids,
    highlights::Highlights,
    pipelines::{blend, cell_fill, cursor, emoji, gamma_blit, monochrome},
    texture::Texture,
};
use crate::{
    text::{cache::FontCache, fonts::Fonts},
    ui::Ui,
    util::vec2::Vec2,
};
use std::sync::Arc;
use swash::shape::ShapeContext;
use winit::window::Window;

pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub struct RenderState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    pipelines: Pipelines,
    targets: Targets,
    grids: Grids,
    highlights: Highlights,
    shape_context: ShapeContext,
    font_cache: FontCache,
}

struct Targets {
    monochrome: Texture,
    color: Texture,
    depth: DepthTexture,
}

struct Pipelines {
    cursor: cursor::Pipeline,
    blend: blend::Pipeline,
    cell_fill: cell_fill::Pipeline,
    emoji: emoji::Pipeline,
    gamma_blit: gamma_blit::Pipeline,
    monochrome: monochrome::Pipeline,
}

impl RenderState {
    pub async fn new(window: Arc<Window>, cell_size: Vec2<u32>) -> Self {
        let surface_size: Vec2<u32> = window.inner_size().into();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(window.as_ref()) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::TEXTURE_BINDING_ARRAY
                    | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
                    | wgpu::Features::UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING
                    | wgpu::Features::PUSH_CONSTANTS,
                limits: adapter.limits(),
            },
            None,
        )
        .await
        .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(surface_caps.formats[0]),
            width: surface_size.x,
            height: surface_size.y,
            present_mode: surface_caps.present_modes[0], // Vsync
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let highlights = Highlights::new(&device);
        let grids = Grids::new(&device);

        let target_size = (surface_size / cell_size) * cell_size;
        let targets = Targets {
            monochrome: Texture::target(&device, target_size, TARGET_FORMAT),
            color: Texture::target(&device, target_size, TARGET_FORMAT),
            depth: DepthTexture::new(&device, target_size),
        };

        Self {
            pipelines: Pipelines {
                cursor: cursor::Pipeline::new(&device, &targets.monochrome.view),
                blend: blend::Pipeline::new(&device, &targets.color.view),
                cell_fill: cell_fill::Pipeline::new(
                    &device,
                    highlights.layout(),
                    grids.bind_group_layout(),
                    TARGET_FORMAT,
                ),
                emoji: emoji::Pipeline::new(&device),
                gamma_blit: gamma_blit::Pipeline::new(
                    &device,
                    surface_config.format,
                    &targets.color.view,
                ),
                monochrome: monochrome::Pipeline::new(&device),
            },
            shape_context: ShapeContext::new(),
            font_cache: FontCache::new(),
            grids: Grids::new(&device),
            targets,
            highlights,
            device,
            queue,
            surface,
            surface_config,
        }
    }

    pub fn update(&mut self, ui: &Ui, fonts: &mut Fonts) {
        let cell_size = fonts.metrics().into_pixels().cell_size();
        let target_size = (self.surface_size() / cell_size) * cell_size;
        self.grids.update(
            &self.device,
            &self.queue,
            ui,
            fonts,
            &mut self.font_cache,
            &mut self.shape_context,
        );
        self.highlights.update(ui, &self.device);
        self.pipelines.cursor.update(
            &self.device,
            ui,
            target_size,
            cell_size.into(),
            &self.targets.monochrome.view,
        );
        self.pipelines.monochrome.update(
            &self.device,
            &self.queue,
            &self.font_cache.monochrome,
            self.highlights.layout(),
            self.grids.bind_group_layout(),
        );
        self.pipelines.emoji.update(
            &self.device,
            &self.queue,
            &self.font_cache.emoji,
            self.grids.bind_group_layout(),
        );
        self.pipelines
            .blend
            .update(&self.device, &self.targets.monochrome.view);
    }

    pub fn resize(&mut self, new_size: Vec2<u32>, cell_size: Vec2<u32>) {
        if new_size == Vec2::default() {
            return;
        }

        self.surface_config.width = new_size.x;
        self.surface_config.height = new_size.y;
        self.surface.configure(&self.device, &self.surface_config);

        let target_size = (new_size / cell_size) * cell_size;
        self.targets.monochrome = Texture::target(&self.device, target_size, TARGET_FORMAT);
        self.targets.color = Texture::target(&self.device, target_size, TARGET_FORMAT);
        self.targets.depth = DepthTexture::new(&self.device, target_size);

        self.pipelines.gamma_blit.update(
            &self.device,
            self.surface_config.format,
            &self.targets.color.view,
            target_size,
            new_size,
        );
    }

    pub fn render(&mut self, cell_size: Vec2<u32>) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render encoder"),
            });
        let target_size = self.targets.color.texture.size().into();
        let Some(highlights_bind_group) = self.highlights.bind_group() else {
            return Ok(());
        };

        self.pipelines.cell_fill.render(
            &mut encoder,
            self.grids.front_to_back(),
            &self.targets.color.view,
            &self.targets.depth.view,
            target_size,
            highlights_bind_group,
            cell_size,
            self.highlights.clear_color(),
        );

        self.pipelines.monochrome.render(
            &mut encoder,
            self.grids.front_to_back(),
            &self.targets.monochrome.view,
            &self.targets.depth.view,
            target_size,
            highlights_bind_group,
        );

        self.pipelines
            .blend
            .render(&mut encoder, &self.targets.color.view);

        self.pipelines
            .cursor
            .render(&mut encoder, &self.targets.color.view);

        self.pipelines.emoji.render(
            &mut encoder,
            self.grids.front_to_back(),
            &self.targets.color.view,
            &self.targets.depth.view,
            target_size,
        );

        self.pipelines
            .gamma_blit
            .render(&mut encoder, &output_view, self.highlights.clear_color());

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn rebuild_swap_chain(&mut self, cell_size: Vec2<u32>) {
        self.resize(self.surface_size(), cell_size);
    }

    pub fn clear(&mut self) {
        self.pipelines.emoji.clear();
        self.pipelines.monochrome.clear();
    }

    pub fn surface_size(&self) -> Vec2<u32> {
        Vec2::new(self.surface_config.width, self.surface_config.height)
    }
}
