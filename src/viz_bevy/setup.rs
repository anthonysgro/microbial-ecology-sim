// COLD PATH: Startup system — runs once to initialize simulation,
// spawn camera, sprite, and overlay label.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::grid::world_init;

use super::resources::{
    ActiveOverlay, BevyVizConfig, GridSprite, MainCamera, OverlayLabel, RenderState,
    SimulationState,
};

/// Format the overlay name for the UI label.
fn overlay_label_text(overlay: &ActiveOverlay) -> String {
    match overlay {
        ActiveOverlay::Heat => "Heat".to_string(),
        ActiveOverlay::Chemical(n) => format!("Chemical {n}"),
    }
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
}
