use super::*;
use bevy::ecs::system::RunSystemOnce;

#[test]
fn aura_shader_compiles_through_bevy_imports_without_rewriting_its_directives() {
    use bevy::shader::{Shader, ShaderCache, ShaderDefVal};
    let mut shaders = Assets::<Shader>::default();
    // The imported vertex interface precedes the material in Bevy's composed WGSL.
    let interface = shaders.add(Shader::from_wgsl(
        "#define_import_path bevy_pbr::forward_io\nstruct VertexOutput { @builtin(position) position: vec4<f32>, @location(2) uv: vec2<f32>, @location(5) color: vec4<f32> };",
        "bevy_pbr/forward_io.wgsl",
    ));
    let material = shaders.add(Shader::from_wgsl(
        include_str!("rare_mod_auras.wgsl"), "survivors/rare_mod_auras.wgsl",
    ));
    let mut cache = ShaderCache::<(), ()>::new(Default::default(), wgpu::DownlevelFlags::empty(), |_, _, _| Ok(()));
    cache.set_shader(interface.id(), shaders.get(&interface).unwrap().clone());
    cache.set_shader(material.id(), shaders.get(&material).unwrap().clone());
    let result = cache.get(&(), 0, material.id(), &[ShaderDefVal::UInt("MATERIAL_BIND_GROUP".into(), 3)]);
    assert!(result.is_ok(), "Bevy aura shader composition failed: {result:?}");
}

#[test]
fn every_rare_mod_has_exactly_one_distinct_signature_and_stat_mods_have_none() {
    assert_eq!(
        STYLES.len(),
        Rank::MODS.into_iter().filter(|m| m.is_special()).count()
    );
    for modifier in Rank::MODS {
        let count = STYLES.iter().filter(|s| s.modifier == modifier).count();
        assert_eq!(count, usize::from(modifier.is_special()), "{modifier:?}");
    }
    for (i, a) in STYLES.iter().enumerate() {
        assert!(a.source < PALADIN.len());
        assert!(a.rate > 0.0);
        for b in &STYLES[i + 1..] {
            assert_ne!((a.color, a.accent, a.rate), (b.color, b.accent, b.rate));
        }
    }
    let rank = Rank::Boss
        .with(Rank::BloodLink)
        .with(Rank::Echo)
        .with(Rank::Healer)
        .with(Rank::BloodLink);
    let layers: Vec<_> = rings(rank, 0.6).collect();
    assert_eq!(layers.len(), 3);
    assert!(layers.windows(2).all(|pair| pair[0].1 < pair[1].1));
    assert_eq!(rings(Rank::Armoured, 0.6).count(), 0);
}

#[test]
fn ground_mesh_follows_slopes_and_keeps_uvs_and_winding_valid() {
    let center = Vec3::new(1000.0, 20.0, -2000.0);
    let foot = footprint(center, 1.5, 0.0, |p| {
        Some(center.y + (p.x - center.x) * 0.4)
    });
    for p in &foot.points {
        assert!((p.y - (center.y + (p.x - center.x) * 0.4 + 0.065)).abs() < 0.0001);
    }
    let mut geometry = Geometry::default();
    geometry.push(&foot, center, 0.5);
    assert_eq!(geometry.points.len(), (GRID + 1).pow(2));
    assert_eq!(geometry.indices.len(), GRID * GRID * 6);
    assert!(
        geometry
            .uvs
            .iter()
            .flatten()
            .all(|v| (0.0..=1.0).contains(v))
    );
    for triangle in geometry.indices.chunks_exact(3) {
        let p = [triangle[0], triangle[1], triangle[2]]
            .map(|i| Vec3::from_array(geometry.points[i as usize]));
        assert!((p[1] - p[0]).cross(p[2] - p[0]).y > 0.0);
    }
}

#[test]
fn all_paladin_sources_resolve_to_installed_ground_art_without_copying_assets() {
    let path = benilla_formats::wow_data_or_skip!();
    let mut chain = benilla_formats::open_chain(&path).unwrap();
    let spells = benilla_formats::load_spell_catalog(&mut chain).unwrap();
    let visuals = benilla_formats::load_spell_visual_catalog(&mut chain).unwrap();
    for source in PALADIN {
        let paths = paladin_paths(&spells, &visuals, source);
        assert!(!paths.is_empty(), "paladin spell {source} has no models");
        let mut textures = Vec::new();
        for path in paths {
            let path = if path.to_ascii_lowercase().ends_with(".mdx") {
                format!("{}.m2", &path[..path.len() - 4])
            } else {
                path
            };
            let Ok(bytes) = chain.read_file(&path) else {
                continue;
            };
            let Ok(parts) = benilla_formats::parse_m2_render_submeshes(&bytes, "", &[]) else {
                continue;
            };
            for part in parts {
                if part.ground_quad().is_some() {
                    if let Some(texture) = part.texture {
                        textures.push(texture);
                    }
                }
            }
        }
        assert!(
            !textures.is_empty(),
            "paladin spell {source} has no ground texture"
        );
        for texture in textures {
            benilla_formats::blp_to_rgba(&chain.read_file(&texture).unwrap()).unwrap();
        }
    }
}

#[test]
fn cosmetic_batches_follow_units_clear_on_death_and_never_change_combat_state() {
    let mut world = World::new();
    super::super::geometry::init_test_world(&mut world);
    world.init_resource::<Run>();
    world.insert_resource(State::new(Phase::Playing));
    world.init_resource::<Scene>();
    world.init_resource::<Art>();
    world.init_resource::<Assets<Mesh>>();
    world.init_resource::<Assets<AuraMaterial>>();
    world.init_resource::<Assets<M2Model>>();
    world.spawn((Hero { vy: 0.0, air: 0.0 }, Transform::default()));
    let enemy = world
        .run_system_once(|mut commands: Commands, mut run: ResMut<Run>| {
            super::super::enemies::spawn_enemy(
                &mut commands,
                &mut run,
                &super::super::data::roster(0).enemies[0],
                Vec3::X * 2.0,
                Vec3::ZERO,
                false,
            )
        })
        .unwrap();
    world.entity_mut(enemy).insert((
        Rank::Healer.with(Rank::BloodLink),
        InheritedVisibility::VISIBLE,
    ));
    let hp = world.get::<Enemy>(enemy).unwrap().hp;
    world.run_system_once(draw).unwrap();
    assert_eq!(world.resource::<Scene>().feet.len(), 2);
    assert_eq!(
        world
            .query::<(&AuraBatch, &Visibility)>()
            .iter(&world)
            .filter(|(_, v)| **v == Visibility::Visible)
            .count(),
        2
    );
    assert_eq!(world.get::<Enemy>(enemy).unwrap().hp, hp);
    let old = world
        .resource::<Scene>()
        .feet
        .values()
        .next()
        .unwrap()
        .center;
    world.get_mut::<Transform>(enemy).unwrap().translation.x += 2.0;
    world.run_system_once(draw).unwrap();
    assert!(
        world
            .resource::<Scene>()
            .feet
            .values()
            .all(|foot| foot.center.x > old.x)
    );
    world.get_mut::<Enemy>(enemy).unwrap().dead = true;
    world.run_system_once(draw).unwrap();
    assert!(world.resource::<Scene>().feet.is_empty());
    assert!(
        world
            .query::<(&AuraBatch, &Visibility)>()
            .iter(&world)
            .all(|(_, v)| *v == Visibility::Hidden)
    );
    assert_eq!(world.resource::<Assets<AuraMaterial>>().len(), STYLES.len());
    let replica = world
        .spawn((
            Rank::Echo,
            Transform::from_xyz(3.0, 0.0, 0.0),
            super::super::unit_frames::Status {
                name: "Guest-visible monster".into(),
                health: 50.0,
                maximum: 50.0,
                level: 10,
                friendly: false,
                debuffs: vec![],
                debuff_stacks: vec![],
            },
        ))
        .id();
    world.run_system_once(draw).unwrap();
    assert_eq!(
        world.resource::<Scene>().feet.len(),
        1,
        "guest replicas have no Enemy component"
    );
    world
        .get_mut::<super::super::unit_frames::Status>(replica)
        .unwrap()
        .health = 0.0;
    world.run_system_once(draw).unwrap();
    assert!(world.resource::<Scene>().feet.is_empty());
    world
        .resource_mut::<Run>()
        .monster_mods
        .grant(Rank::BloodLink, 0.0, Some(5.0), Vec3::ZERO);
    world.run_system_once(draw).unwrap();
    assert_eq!(
        world.resource::<Scene>().feet.len(),
        1,
        "stolen mods have the same aura on the local player"
    );
    world.insert_resource(State::new(Phase::Hub));
    world.run_system_once(draw).unwrap();
    assert!(world.resource::<Scene>().feet.is_empty());
    assert_eq!(world.resource::<Assets<AuraMaterial>>().len(), STYLES.len());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn shader_renders_all_distinct_motifs_on_the_gpu() {
    use wgpu::util::DeviceExt;
    futures_lite::future::block_on(async {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let Ok(adapter) = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
        else {
            eprintln!("SKIP: no GPU adapter for rare-mod aura rendering");
            return;
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rare mod aura validation"),
                ..Default::default()
            })
            .await
            .unwrap();
        let source = include_str!("rare_mod_auras.wgsl")
            .replace("#import bevy_pbr::forward_io::VertexOutput", "")
            .replace("#{MATERIAL_BIND_GROUP}", "0");
        let source = format!(
            "{}\n{}",
            r#"
struct VertexOutput { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };
@vertex fn preview_vertex(@builtin(vertex_index) i:u32)->VertexOutput {
    let p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));
    var o:VertexOutput; o.position=vec4<f32>(p[i],0.0,1.0); o.uv=p[i]*0.5+0.5; o.color=vec4<f32>(0.17,1.0,1.0,1.0); return o;
}
"#,
            source
        );
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rare-mod aura shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rare aura preview"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("preview_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let shader_error = device.pop_error_scope().await;
        assert!(
            shader_error.is_none(),
            "rare-mod shader compilation: {shader_error:?}"
        );
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        const TILE: u32 = 192;
        const SIDE: u32 = TILE * 6;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rare aura atlas"),
            size: wgpu::Extent3d {
                width: SIDE,
                height: SIDE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let white = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            white.as_image_copy(),
            &[255; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_view = white.create_view(&Default::default());
        let sampler = device.create_sampler(&Default::default());
        let bind_groups: Vec<_> = STYLES
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let a = Color::srgb_from_array(s.color)
                    .to_linear()
                    .to_vec4()
                    .to_array();
                let b = Color::srgb_from_array(s.accent)
                    .to_linear()
                    .to_vec4()
                    .to_array();
                let data = [
                    a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3], 2.3, i as f32, s.rate, 0.0,
                ];
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&data),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&white_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                })
            })
            .collect();
        let mut encoder = device.create_command_encoder(&Default::default());
        let view = target.create_view(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("34 rare aura motifs"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.012,
                            g: 0.016,
                            b: 0.024,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&pipeline);
            for (i, group) in bind_groups.iter().enumerate() {
                pass.set_viewport(
                    (i as u32 % 6 * TILE) as f32,
                    (i as u32 / 6 * TILE) as f32,
                    TILE as f32,
                    TILE as f32,
                    0.0,
                    1.0,
                );
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (SIDE * SIDE * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIDE * 4),
                    rows_per_image: Some(SIDE),
                },
            },
            wgpu::Extent3d {
                width: SIDE,
                height: SIDE,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            tx.send(r).unwrap();
        });
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        assert!(
            device.pop_error_scope().await.is_none(),
            "rare-mod shader failed GPU validation"
        );
        let data = read.slice(..).get_mapped_range();
        let mut signatures = HashSet::new();
        for i in 0..STYLES.len() {
            let mut signature = 0xcbf29ce484222325_u64;
            let mut lit = 0;
            for y in 0..TILE {
                for x in 0..TILE {
                    let at = (((i as u32 / 6 * TILE + y) * SIDE + i as u32 % 6 * TILE + x) * 4 + 3)
                        as usize;
                    let alpha = data[at];
                    lit += usize::from(alpha > 8);
                    signature = (signature ^ alpha as u64).wrapping_mul(0x100000001b3);
                }
            }
            assert!(lit > 300, "motif {i} has no readable silhouette");
            assert!(
                signatures.insert(signature),
                "motif {i} repeats another silhouette"
            );
        }
        if let Some(path) = std::env::var_os("RARE_MOD_AURA_PREVIEW") {
            let mut rgba = data.to_vec();
            for pixel in rgba.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            image::save_buffer(path, &rgba, SIDE, SIDE, image::ColorType::Rgba8).unwrap();
        }
    });
}
