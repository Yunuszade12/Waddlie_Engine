use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::gizmos::config::GizmoConfigStore;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{Atmosphere, AtmospherePlugin, ScatteringMedium};
use bevy::prelude::*;
use bevy::state::commands;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// 🚀 WASM-BINDGEN INTEROP CONFIGURATION
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    // Bridges a native browser JavaScript routine to invoke an on-the-fly download layer
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn js_console_log(s: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
    #[default]
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedAxis {
    X,
    Y,
    Z,
    None,
}

#[derive(Resource, Default)]
pub struct EditorSelection {
    pub selected: Option<Entity>,
    pub mode: GizmoMode,
    pub active_axis: Option<SelectedAxis>,
    pub initial_drag_value: Option<Vec3>,
    pub last_intersect_point: Option<Vec3>,
    pub backup_translation: Option<Vec3>,
    pub backup_rotation: Option<Quat>,
    pub backup_scale: Option<Vec3>,
    pub is_local: bool, // 🚀 NEW: Tracks if we use Local (true) or Global (false) coordinates
}

#[derive(Component)]
pub struct EditorCamera;

#[derive(Component)]
struct Sun;

#[derive(Component)]
pub struct SceneEntity {
    pub json_id: u32,
}

#[derive(Resource)]
pub struct CurrentSceneHandle(pub Handle<SceneAsset>);

#[derive(Resource, Default)]
pub struct LoadedSceneData {
    pub items: Vec<SceneJsonDeserialize>,
}

#[derive(Resource, Default)]
pub struct SceneSpawnStatus {
    pub spawned: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum SceneJsonDeserialize {
    Entity {
        name: String,
        position_x: f32,
        position_y: f32,
        position_z: f32,
        rotation_x: f32,
        rotation_y: f32,
        rotation_z: f32,
        scale_x: f32,
        scale_y: f32,
        scale_z: f32,
        color_rgb: [f32; 3],
        components: Vec<JsonComponentKind>,
        parent_id: Option<u32>,
        id: u32,
    },
    DirectionalLight {
        morningcolor: [f32; 3],
        nightcolor: [f32; 3],
    },
    DayNightCycle {
        active: bool,
        time: f32,
        speed: f32,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum JsonComponentKind {
    Mesh { mesh_type: String },
    Material { color_rgb: [f32; 3] },
    GltfModel { path: String },
    ExternalComponent { file_name: String },
    ProceduralSky,
    ImageSkybox { path: String, brightness: f32 },
}

#[derive(Asset, TypePath, Clone, Debug)]
pub struct SceneAsset {
    pub items: Vec<SceneJsonDeserialize>,
}

// 🚀 FIX: Add TypePath to the derive list
#[derive(Default, TypePath)]
pub struct SceneAssetLoader;

#[derive(Debug)]
pub enum SceneAssetLoaderError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for SceneAssetLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Asset loading IO error: {}", e),
            Self::Json(e) => write!(f, "Asset JSON parsing error: {}", e),
        }
    }
}
impl std::error::Error for SceneAssetLoaderError {}

impl AssetLoader for SceneAssetLoader {
    type Asset = SceneAsset;
    type Settings = ();
    type Error = SceneAssetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(SceneAssetLoaderError::Io)?;
        let items = serde_json::from_slice(&bytes).map_err(SceneAssetLoaderError::Json)?;
        Ok(SceneAsset { items })
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

pub fn boot_editor_base(app: &mut App) {
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Waddlie Engine".to_string(),
            canvas: Some("#bevy-canvas".to_string()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: false,
            ..default()
        }),
        ..default()
    }));

    app.init_asset::<SceneAsset>();
    app.init_asset_loader::<SceneAssetLoader>();

    app.init_resource::<EditorSelection>();
    app.init_resource::<LoadedSceneData>();
    app.init_resource::<SceneSpawnStatus>();
    app.insert_resource(ClearColor(Color::srgba(0.5, 0.7, 0.9, 1.0)));

    app.add_systems(Startup, setup_editor_environment);
    app.add_systems(Update, load_and_construct_editor_scene);

    app.add_systems(
        Update,
        (
            editor_camera_fly_system,
            gizmo_mode_switch_system,
            editor_gizmo_interaction_system,
            render_native_gizmos_system,
        ),
    );
}

fn setup_editor_environment(
    mut commands: Commands,
    mut gizmo_config_store: ResMut<GizmoConfigStore>,
    asset_server: Res<AssetServer>,
) {
    if let Some((_, config, _)) = gizmo_config_store.iter_mut().next() {
        config.depth_bias = -1.0;
    }

    let handle = asset_server.load::<SceneAsset>("scene.json");
    commands.insert_resource(CurrentSceneHandle(handle));

    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(0.0, 5.0, 10.0),
        EditorCamera,
    ));

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 500.0, 0.0),
        Sun,
    ));

    commands.spawn(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        ..default()
    });
}

fn load_and_construct_editor_scene(
    mut commands: Commands,
    scene_handle: Option<Res<CurrentSceneHandle>>,
    scene_assets: Res<Assets<SceneAsset>>,
    mut spawn_status: ResMut<SceneSpawnStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
    asset_server: Res<AssetServer>,
    mut scene_data_res: ResMut<LoadedSceneData>,
) {
    if spawn_status.spawned {
        return;
    }
    let Some(handle) = scene_handle else {
        return;
    };
    let Some(scene_asset) = scene_assets.get(&handle.0) else {
        return;
    };

    info!("Scene asset downloaded successfully! Spawning world items...");
    let world = scene_asset.items.clone();
    scene_data_res.items = world.clone();

    let mut id_to_entity_map: HashMap<u32, Entity> = HashMap::new();
    let mut parent_child_relations: Vec<(Entity, u32)> = Vec::new();

    for scene_item in world {
        if let SceneJsonDeserialize::Entity {
            id,
            name,
            position_x,
            position_y,
            position_z,
            rotation_x,
            rotation_y,
            rotation_z,
            scale_x,
            scale_y,
            scale_z,
            components,
            parent_id,
            ..
        } = scene_item
        {
            let spawned_entity_id = commands
                .spawn((
                    Name::new(name),
                    Transform {
                        translation: Vec3::new(position_x, position_y, position_z),
                        rotation: Quat::from_euler(
                            EulerRot::XYZ,
                            rotation_x.to_radians(),
                            rotation_y.to_radians(),
                            rotation_z.to_radians(),
                        ),
                        scale: Vec3::new(scale_x, scale_y, scale_z),
                    },
                    Visibility::default(),
                    SceneEntity { json_id: id },
                ))
                .id();

            id_to_entity_map.insert(id, spawned_entity_id);
            if let Some(pid) = parent_id {
                parent_child_relations.push((spawned_entity_id, pid));
            }

            for component in components {
                match component {
                    JsonComponentKind::Mesh { mesh_type } => {
                        if mesh_type == "camera" {
                            continue;
                        }
                        let mesh_handle = match mesh_type.as_str() {
                            "sphere" => meshes.add(Sphere::new(0.5)),
                            _ => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                        };
                        commands
                            .entity(spawned_entity_id)
                            .insert(Mesh3d(mesh_handle));
                    }
                    JsonComponentKind::Material { color_rgb } => {
                        let mat_handle = materials.add(StandardMaterial {
                            base_color: Color::linear_rgb(color_rgb[0], color_rgb[1], color_rgb[2]),
                            ..default()
                        });
                        commands
                            .entity(spawned_entity_id)
                            .insert(MeshMaterial3d(mat_handle));
                    }
                    JsonComponentKind::GltfModel { path } => {
                        let s_path = format!("{}#Scene0", path);
                        let model_child = commands
                            .spawn((
                                SceneRoot(asset_server.load(s_path)),
                                Transform::default(),
                                Visibility::default(),
                            ))
                            .id();
                        commands.entity(spawned_entity_id).add_child(model_child);
                    }
                    JsonComponentKind::ProceduralSky => {
                        let medium_handle = scattering_mediums.add(ScatteringMedium::default());
                        commands
                            .entity(spawned_entity_id)
                            .insert(Atmosphere::earthlike(medium_handle));
                    }
                    _ => {}
                }
            }
        }
    }

    for (child_entity, parent_json_id) in parent_child_relations {
        if let Some(&parent_entity) = id_to_entity_map.get(&parent_json_id) {
            commands.entity(child_entity).insert(ChildOf(parent_entity));
        }
    }

    spawn_status.spawned = true;
}

fn gizmo_mode_switch_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<EditorSelection>,
) {
    if keyboard_input.just_pressed(KeyCode::Digit1) {
        selection.mode = GizmoMode::Translate;
    } else if keyboard_input.just_pressed(KeyCode::Digit2) {
        selection.mode = GizmoMode::Rotate;
    } else if keyboard_input.just_pressed(KeyCode::Digit3) {
        selection.mode = GizmoMode::Scale;
    }
}

fn editor_gizmo_interaction_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    targets_query: Query<(Entity, &GlobalTransform), With<SceneEntity>>,
    mut transforms_query: Query<(&mut Transform, &SceneEntity)>,
    mut selection: ResMut<EditorSelection>,
    mut scene_data: ResMut<LoadedSceneData>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let cursor_pos = match window.cursor_position() {
        Some(pos) => pos,
        None => return,
    };

    let ray = match camera.viewport_to_world(camera_transform, cursor_pos) {
        Ok(r) => r,
        Err(_) => return,
    };

    // 🚀 NEW: Toggle Local vs Global coordinates with 'J'
    if keyboard_input.just_pressed(KeyCode::KeyJ) {
        selection.is_local = !selection.is_local;
        info!(
            "Coordinate System Switched to: {}",
            if selection.is_local {
                "LOCAL"
            } else {
                "GLOBAL"
            }
        );
    }

    // 1. SELECT AN ENTITY
    if selection.active_axis.is_none() {
        if mouse_input.just_pressed(MouseButton::Left) && !mouse_input.pressed(MouseButton::Right) {
            let mut closest_hit: Option<(Entity, f32)> = None;

            for (entity, global_transform) in targets_query.iter() {
                let pos = global_transform.translation();
                let distance = ray.origin.distance(pos);
                let to_object = pos - ray.origin;
                let projection = to_object.dot(*ray.direction);

                if projection > 0.0 {
                    let closest_point_on_ray = ray.origin + *ray.direction * projection;
                    if closest_point_on_ray.distance(pos) < 1.5 {
                        if closest_hit.is_none() || distance < closest_hit.unwrap().1 {
                            closest_hit = Some((entity, distance));
                        }
                    }
                }
            }

            if closest_hit.is_some() {
                selection.selected = closest_hit.map(|(e, _)| e);
                selection.mode = GizmoMode::None;
            } else {
                selection.selected = None;
                selection.mode = GizmoMode::None;
            }
        }
    }

    // 2. ACTIVATE GIZMO MODES (G, R, or F)
    if let Some(selected_entity) = selection.selected {
        if selection.active_axis.is_none() {
            if let Ok((transform, _)) = transforms_query.get(selected_entity) {
                let mut chosen_mode = GizmoMode::None;

                if keyboard_input.just_pressed(KeyCode::KeyG) {
                    chosen_mode = GizmoMode::Translate;
                } else if keyboard_input.just_pressed(KeyCode::KeyR) {
                    chosen_mode = GizmoMode::Rotate;
                } else if keyboard_input.just_pressed(KeyCode::KeyF) {
                    chosen_mode = GizmoMode::Scale;
                }

                if chosen_mode != GizmoMode::None {
                    selection.mode = chosen_mode;
                    selection.last_intersect_point =
                        Some(Vec3::new(cursor_pos.x, cursor_pos.y, 0.0));
                    selection.backup_translation = Some(transform.translation);
                    selection.backup_rotation = Some(transform.rotation);
                    selection.backup_scale = Some(transform.scale);
                    selection.active_axis = Some(SelectedAxis::None);
                }
            }
        }
    }

    // 3. LOCK AXIS (X, Y, Z)
    if selection.active_axis.is_some() {
        let mut axis_changed = false;
        let mut new_axis = selection.active_axis.unwrap();

        if keyboard_input.just_pressed(KeyCode::KeyX) {
            new_axis = SelectedAxis::X;
            axis_changed = true;
        } else if keyboard_input.just_pressed(KeyCode::KeyY) {
            new_axis = SelectedAxis::Y;
            axis_changed = true;
        } else if keyboard_input.just_pressed(KeyCode::KeyZ) {
            new_axis = SelectedAxis::Z;
            axis_changed = true;
        }

        if axis_changed {
            selection.active_axis = Some(new_axis);
            if let Some(selected_entity) = selection.selected {
                if let Ok((mut transform, _)) = transforms_query.get_mut(selected_entity) {
                    if let Some(pos) = selection.backup_translation {
                        transform.translation = pos;
                    }
                    if let Some(rot) = selection.backup_rotation {
                        transform.rotation = rot;
                    }
                    if let Some(scl) = selection.backup_scale {
                        transform.scale = scl;
                    }
                }
            }
            selection.last_intersect_point = Some(Vec3::new(cursor_pos.x, cursor_pos.y, 0.0));
        }
    }

    // 4. APPLY TRANSFORMATIONS
    if selection.active_axis.is_some() {
        if let Some(selected_entity) = selection.selected {
            if let Ok((mut transform, scene_entity)) = transforms_query.get_mut(selected_entity) {
                let active_axis = selection.active_axis.unwrap();
                let start_mouse = selection.last_intersect_point.unwrap_or(Vec3::ZERO);
                let delta_x = cursor_pos.x - start_mouse.x;

                if active_axis == SelectedAxis::None {
                    // 🚀 UNCONSTRAINED/VIEW-SPACE MANIPULATION
                    match selection.mode {
                        GizmoMode::Translate => {
                            let plane_normal = *camera_transform.forward();
                            let denom = ray.direction.dot(plane_normal);
                            if denom.abs() > 1e-5 {
                                let origin_ref = selection
                                    .backup_translation
                                    .unwrap_or(transform.translation);
                                let t = (origin_ref - ray.origin).dot(plane_normal) / denom;
                                if t > 0.0 {
                                    let current_intersect = ray.origin + *ray.direction * t;
                                    if let Some(init_pos) = selection.backup_translation {
                                        transform.translation =
                                            init_pos + (current_intersect - origin_ref);
                                    }
                                }
                            }
                        }
                        GizmoMode::Rotate => {
                            if let Some(initial_rot) = selection.backup_rotation {
                                // 🚀 FIXED: Rotates relative to the screen look-direction (view space)
                                let sensitivity = 0.005;
                                let angle = delta_x * sensitivity;
                                let view_rotation =
                                    Quat::from_axis_angle(*camera_transform.forward(), angle);
                                transform.rotation = view_rotation * initial_rot;
                            }
                        }
                        GizmoMode::Scale => {
                            if let Some(initial_scale) = selection.backup_scale {
                                // 🚀 FIXED: Uniform scaling scaling everything in proportion
                                let sensitivity = 0.005;
                                let percentage = (1.0 + delta_x * sensitivity).max(0.05);
                                transform.scale = initial_scale * percentage;
                            }
                        }
                        _ => {}
                    }
                } else {
                    // 🚀 CONSTRAINED AXIS MANIPULATION (LOCAL VS GLOBAL LOGIC)
                    match selection.mode {
                        GizmoMode::Translate => {
                            if let Some(initial_pos) = selection.backup_translation {
                                let sensitivity = 0.05;
                                let amount = delta_x * sensitivity;

                                if selection.is_local {
                                    // Move along the object's own local direction vectors
                                    let local_dir = match active_axis {
                                        SelectedAxis::X => transform.local_x(),
                                        SelectedAxis::Y => transform.local_y(),
                                        SelectedAxis::Z => transform.local_z(),
                                        _ => Dir3::X,
                                    };
                                    transform.translation = initial_pos + *local_dir * amount;
                                } else {
                                    // World Space Coordinates
                                    match active_axis {
                                        SelectedAxis::X => {
                                            transform.translation.x = initial_pos.x + amount
                                        }
                                        SelectedAxis::Y => {
                                            transform.translation.y = initial_pos.y + amount
                                        }
                                        SelectedAxis::Z => {
                                            transform.translation.z = initial_pos.z + amount
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        GizmoMode::Rotate => {
                            if let Some(initial_rot) = selection.backup_rotation {
                                let sensitivity = 0.005;
                                let angle = delta_x * sensitivity;

                                if selection.is_local {
                                    // Rotate around local axis vectors
                                    match active_axis {
                                        SelectedAxis::X => {
                                            transform.rotation =
                                                initial_rot * Quat::from_rotation_x(angle)
                                        }
                                        SelectedAxis::Y => {
                                            transform.rotation =
                                                initial_rot * Quat::from_rotation_y(angle)
                                        }
                                        SelectedAxis::Z => {
                                            transform.rotation =
                                                initial_rot * Quat::from_rotation_z(angle)
                                        }
                                        _ => {}
                                    }
                                } else {
                                    // Rotate around global world axis vectors
                                    match active_axis {
                                        SelectedAxis::X => {
                                            transform.rotation =
                                                Quat::from_rotation_x(angle) * initial_rot
                                        }
                                        SelectedAxis::Y => {
                                            transform.rotation =
                                                Quat::from_rotation_y(angle) * initial_rot
                                        }
                                        SelectedAxis::Z => {
                                            transform.rotation =
                                                Quat::from_rotation_z(angle) * initial_rot
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        GizmoMode::Scale => {
                            if let Some(initial_scale) = selection.backup_scale {
                                let sensitivity = 0.005;
                                let percentage = (1.0 + delta_x * sensitivity).max(0.05);
                                // Note: Scale is fundamentally an object-local property in Bevy
                                match active_axis {
                                    SelectedAxis::X => {
                                        transform.scale.x = initial_scale.x * percentage
                                    }
                                    SelectedAxis::Y => {
                                        transform.scale.y = initial_scale.y * percentage
                                    }
                                    SelectedAxis::Z => {
                                        transform.scale.z = initial_scale.z * percentage
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // 5. CONFIRMATION OR CANCEL
                if mouse_input.just_pressed(MouseButton::Left) {
                    selection.active_axis = None;
                    selection.mode = GizmoMode::None;

                    let (euler_x, euler_y, euler_z) = transform.rotation.to_euler(EulerRot::XYZ);
                    for item in scene_data.items.iter_mut() {
                        if let &mut SceneJsonDeserialize::Entity {
                            ref id,
                            ref mut position_x,
                            ref mut position_y,
                            ref mut position_z,
                            ref mut rotation_x,
                            ref mut rotation_y,
                            ref mut rotation_z,
                            ref mut scale_x,
                            ref mut scale_y,
                            ref mut scale_z,
                            ..
                        } = item
                        {
                            if *id == scene_entity.json_id {
                                *position_x = transform.translation.x;
                                *position_y = transform.translation.y;
                                *position_z = transform.translation.z;
                                *rotation_x = euler_x.to_degrees();
                                *rotation_y = euler_y.to_degrees();
                                *rotation_z = euler_z.to_degrees();
                                *scale_x = transform.scale.x;
                                *scale_y = transform.scale.y;
                                *scale_z = transform.scale.z;
                                break;
                            }
                        }
                    }
                    trigger_web_scene_download(&scene_data.items);
                }

                if mouse_input.just_pressed(MouseButton::Right)
                    || keyboard_input.just_pressed(KeyCode::Escape)
                {
                    if let Some(pos) = selection.backup_translation {
                        transform.translation = pos;
                    }
                    if let Some(rot) = selection.backup_rotation {
                        transform.rotation = rot;
                    }
                    if let Some(scl) = selection.backup_scale {
                        transform.scale = scl;
                    }
                    selection.active_axis = None;
                    selection.mode = GizmoMode::None;
                }
            }
        }
    }
}

fn render_native_gizmos_system(
    selection: Res<EditorSelection>,
    global_transforms: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    if let Some(selected_entity) = selection.selected {
        if let Ok(global_transform) = global_transforms.get(selected_entity) {
            let pos = global_transform.translation();

            let color_x = Color::srgb(1.0, 0.1, 0.1);
            let color_y = Color::srgb(0.1, 1.0, 0.1);
            let color_z = Color::srgb(0.1, 0.1, 1.0);

            if let Some(axis) = selection.active_axis {
                match axis {
                    SelectedAxis::X => {
                        gizmos.line(pos - Vec3::X * 50.0, pos + Vec3::X * 50.0, color_x)
                    }
                    SelectedAxis::Y => {
                        gizmos.line(pos - Vec3::Y * 50.0, pos + Vec3::Y * 50.0, color_y)
                    }
                    SelectedAxis::Z => {
                        gizmos.line(pos - Vec3::Z * 50.0, pos + Vec3::Z * 50.0, color_z)
                    }
                    SelectedAxis::None => {
                        gizmos.line(
                            pos - Vec3::X * 2.0,
                            pos + Vec3::X * 2.0,
                            color_x.with_alpha(0.4),
                        );
                        gizmos.line(
                            pos - Vec3::Y * 2.0,
                            pos + Vec3::Y * 2.0,
                            color_y.with_alpha(0.4),
                        );
                        gizmos.line(
                            pos - Vec3::Z * 2.0,
                            pos + Vec3::Z * 2.0,
                            color_z.with_alpha(0.4),
                        );
                    }
                }
            } else {
                gizmos.cube(
                    Transform::from_translation(pos + Vec3::X * 1.0)
                        .with_scale(Vec3::new(2.0, 0.05, 0.05)),
                    color_x,
                );
                gizmos.cube(
                    Transform::from_translation(pos + Vec3::Y * 1.0)
                        .with_scale(Vec3::new(0.05, 2.0, 0.05)),
                    color_y,
                );
                gizmos.cube(
                    Transform::from_translation(pos + Vec3::Z * 1.0)
                        .with_scale(Vec3::new(0.05, 0.05, 2.0)),
                    color_z,
                );
            }
        }
    }
}

fn editor_camera_fly_system(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut camera_query: Query<&mut Transform, With<EditorCamera>>,
) {
    let delta = mouse_motion.delta;
    for mut cam_transform in camera_query.iter_mut() {
        if mouse_input.pressed(MouseButton::Right) && delta.length_squared() > 0.0 {
            let sensitivity = 0.002;
            let (mut yaw, mut pitch, _) = cam_transform.rotation.to_euler(EulerRot::YXZ);
            yaw -= delta.x * sensitivity;
            pitch -= delta.y * sensitivity;
            pitch = pitch.clamp(-1.4, 1.4);
            cam_transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
        }

        let mut move_direction = Vec3::ZERO;
        let speed = 20.0;
        let forward = cam_transform.forward();
        let right = cam_transform.right();

        if keyboard_input.pressed(KeyCode::KeyW) {
            move_direction += *forward;
        }
        if keyboard_input.pressed(KeyCode::KeyS) {
            move_direction -= *forward;
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            move_direction -= *right;
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            move_direction += *right;
        }
        if keyboard_input.pressed(KeyCode::Space) {
            move_direction += Vec3::Y;
        }
        if keyboard_input.pressed(KeyCode::ShiftLeft) {
            move_direction -= Vec3::Y;
        }

        if move_direction.length_squared() > 0.0 {
            cam_transform.translation += move_direction.normalize() * speed * time.delta_secs();
        }
    }
}

pub fn trigger_web_scene_download(scene_data: &Vec<SceneJsonDeserialize>) {
    let json_string = serde_json::to_string(&scene_data).unwrap(); // Use compact string for storage space

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(local_storage)) = window.local_storage() {
                // Saves the JSON directly into browser sandbox RAM
                if let Ok(_) = local_storage.set_item("waddlie_current_scene", &json_string) {
                    js_console_log("Wasm Interop: Scene auto-saved to browser LocalStorage RAM.");
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("Desktop Auto-Save: {}", json_string);
    }
}
