use super::{
    state::{GridInfo, StateConstant},
    state_surface_config::StateSurfaceConfig,
    texture::Texture,
};
use crate::text::cache::FontCache;
use bytemuck::cast_slice;
use std::num::NonZeroU32;
use wgpu::{include_wgsl, util::DeviceExt};

// TODO: Resizable buffer

pub struct StateFontConstant {
    pub sampler: wgpu::Sampler,
    pub glyph_shader: wgpu::ShaderModule,
}

pub struct StateFontRead {
    pub bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::RenderPipeline,
}

pub struct StateFontWrite {
    pub font_cache: FontCache,
    textures: Vec<Texture>,
    next_glyph_to_upload: usize,
}

impl StateFontWrite {
    pub fn get_read(
        &mut self,
        constant: &StateConstant,
        surface_config: &StateSurfaceConfig,
    ) -> StateFontRead {
        // TODO: Only update pipeline if there are textures to upload

        for (data, info) in self
            .font_cache
            .data
            .iter()
            .zip(self.font_cache.info.iter())
            .skip(self.next_glyph_to_upload)
        {
            self.textures.push(Texture::new(
                &constant.device,
                &constant.queue,
                data.as_slice(),
                info.size,
            ));
        }

        self.next_glyph_to_upload = self.textures.len();

        // TODO: Cache this array
        let views: Vec<_> = self.textures.iter().map(|texture| &texture.view).collect();

        let tex_count = Some(NonZeroU32::new(self.textures.len() as u32).unwrap());
        let font_bind_group_layout =
            constant
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Texture bind group layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: tex_count,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let font_info_buffer =
            constant
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Font info buffer"),
                    contents: cast_slice(self.font_cache.info.as_slice()),
                    usage: wgpu::BufferUsages::STORAGE,
                });

        let glyph_pipeline_layout =
            constant
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[
                        &constant.highlights_bind_group_layout,
                        &constant.grid_bind_group_layout,
                        &font_bind_group_layout,
                    ],
                    push_constant_ranges: &[wgpu::PushConstantRange {
                        stages: wgpu::ShaderStages::VERTEX,
                        range: 0..GridInfo::SIZE as u32,
                    }],
                });

        let glyph_render_pipeline =
            constant
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Render pipeline"),
                    layout: Some(&glyph_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &constant.font.glyph_shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &constant.font.glyph_shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: surface_config.format(),
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    // How to interpret vertices when converting to triangles
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                });

        StateFontRead {
            bind_group: constant
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Font bind group"),
                    layout: &font_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureViewArray(views.as_slice()),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&constant.font.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &font_info_buffer,
                                offset: 0,
                                size: None,
                            }),
                        },
                    ],
                }),
            pipeline: glyph_render_pipeline,
        }
    }
}

pub fn new(device: &wgpu::Device) -> (StateFontWrite, StateFontConstant) {
    (
        StateFontWrite {
            font_cache: FontCache::new(),
            textures: vec![],
            next_glyph_to_upload: 0,
        },
        StateFontConstant {
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Texture sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            }),
            glyph_shader: device.create_shader_module(include_wgsl!("glyph.wgsl")),
        },
    )
}
