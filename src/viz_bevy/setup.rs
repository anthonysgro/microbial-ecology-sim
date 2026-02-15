// COLD PATH: Startup system — runs once to initialize simulation,
// spawn camera, sprite, and overlay label.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::grid::world_init;

use super::resources::{
    ActiveOverlay, BevyVizConfig, GridSprite, HoverTooltip, MainCamera, OverlayLabel,
    RenderState, ScaleBar, ScaleMaxLabel, ScaleMinLabel, SimulationState,
};

/// Format the overlay name for the UI label.
pub(super) fn overlay_label_text(overlay: &ActiveOverlay) -> String {
    match overlay {
        ActiveOverlay::Heat => "Heat".to_string(),
        ActiveOverlay::Chemical(n) => format!("Chemical {n}"),
    }
}

/// Build a vertical gradient image for the color scale bar.
///
/// Top of the image = normalized 1.0 (max), bottom = 0.0 (min).
/// Uses the appropriate color function for the given overlay.
pub(super) fn build_scale_image(
    width: u32,
    height: u32,
    overlay: &ActiveOverlay,
) -> Image {
    use super::color::{chemical_color_rgba, heat_color_rgba};

    let color_fn: fn(f32) -> [u8; 4] = match overlay {
        ActiveOverlay::Heat => heat_color_rgba,
        ActiveOverlay::Chemical(_) => chemical_color_rgba,
    };

    let mut data = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        // Top row = 1.0, bottom row = 0.0.
        let normalized = 1.0 - (y as f32 / (height - 1).max(1) as f32);
        let rgba = color_fn(normalized);
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            data[offset] = rgba[0];
            data[offset + 1] = rgba[1];
            data[offset + 2] = rgba[2];
            data[offset + 3] = rgba[3];
        }
    }

    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// Startup system: initializes the simulation, pre-allocates render buffers,
/// creates the GPU texture, and spawns camera + sprite + label entities.
///
/// Consumes `BevyVizConfig` (read-only) to derive all initialization parameters.
/// Panics on world-init failure — this is a COLD one-shot path where `expect`
/// is acceptable per project conventions.
pub fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    config: Res<BevyVizConfig>,
) {
    // ── Initialize simulation ──────────────────────────────────────
    let grid = world_init::initialize(
        config.seed,
        config.grid_config.clone(),
        &config.init_config,
        config.actor_config.clone(),
    )
    .expect("world initialization failed during Bevy setup");

    let width = grid.width();
    let height = grid.height();
    let cell_count = grid.cell_count();

    // ── Insert simulation resource ─────────────────────────────────
    commands.insert_resource(SimulationState {
        config: config.grid_config.clone(),
        grid,
        tick: 0,
        running: true,
    });

    // ── Pre-allocate render buffers (Req 5.4, 9.2) ─────────────────
    commands.insert_resource(RenderState {
        pixel_buffer: vec![0u8; cell_count * 4],
        norm_buffer: vec![0.0f32; cell_count],
    });

    // ── Insert active overlay from config ──────────────────────────
    commands.insert_resource(config.initial_overlay);

    // ── Create GPU texture (Req 5.1) ───────────────────────────────
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();

    let image_handle = images.add(image);

    // ── Spawn camera (Req 8.1) ─────────────────────────────────────
    commands.spawn((Camera2d, MainCamera));

    // ── Spawn grid sprite (Req 5.3) ────────────────────────────────
    commands.spawn((
        Sprite {
            image: image_handle,
            custom_size: Some(Vec2::new(width as f32, height as f32)),
            ..default()
        },
        GridSprite,
    ));

    // ── Spawn overlay label (Req 7.1, 7.2) ─────────────────────────
    let label_text = overlay_label_text(&config.initial_overlay);
    commands.spawn((
        Text::new(label_text),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        OverlayLabel,
    ));

    // ── Spawn hover tooltip (bottom-left) ──────────────────────────
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        HoverTooltip,
    ));

    // ── Spawn color scale bar (right edge) ─────────────────────────
    let scale_height: u32 = 256;
    let scale_width: u32 = 20;
    let scale_image = build_scale_image(
        scale_width,
        scale_height,
        &config.initial_overlay,
    );
    let scale_handle = images.add(scale_image);

    // Container node for the scale bar + labels, anchored to the right.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top: Val::Px(40.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|parent| {
            // Max label at top of scale
            parent.spawn((
                Text::new(format!("{:.1}", config.color_scale_max)),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ScaleMaxLabel,
            ));

            // Scale bar image
            parent.spawn((
                ImageNode {
                    image: scale_handle,
                    ..default()
                },
                Node {
                    width: Val::Px(scale_width as f32),
                    height: Val::Px(scale_height as f32),
                    ..default()
                },
                ScaleBar,
            ));

            // Min label at bottom of scale
            parent.spawn((
                Text::new("0"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ScaleMinLabel,
            ));
        });
}
