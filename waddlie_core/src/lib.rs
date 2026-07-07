use bevy::asset::io::memory::MemoryAssetReader;
use bevy::asset::{AssetLoader, LoadContext, io::Reader};

use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::ecs::relationship::Relationship;
use bevy::gizmos::config::GizmoConfigStore;
use bevy::gltf::{Gltf, GltfLoader};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{Atmosphere, AtmospherePlugin, ScatteringMedium};
use bevy::prelude::*;
use bevy::state::commands;
use downcast_rs::Downcast;
#[cfg(target_arch = "wasm32")]
use serde::de::value;
use std::io::Cursor;

use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use std::sync::Mutex;
//Const and Sturcts
const GIZMO_LAYER: RenderLayers = RenderLayers::layer(1);
static JS_RIGGING_COMMANDS: Mutex<Vec<(u32, bool)>> = Mutex::new(Vec::new());
pub static INCOMING_ASSETS: Mutex<Vec<(String, Vec<u8>)>> = Mutex::new(Vec::new());
//Hierachy command statics
// Place this near your other static definitions
static HIERARCHY_COMMANDS: Mutex<Vec<WebCommandPayload>> = Mutex::new(Vec::new());

#[derive(bevy::prelude::Resource, Default)]
pub struct WasmAssetCache {
    pub models: std::collections::HashMap<String, Vec<u8>>,
    pub handles: Vec<bevy::prelude::UntypedHandle>,
    //Keeps a direct pipeline to Bevy's virtual RAM filesystem
    pub virtual_dir: Option<bevy::asset::io::memory::Dir>,
}
//wasm bindgen so we can talk with our JavaScript frontend.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn js_console_log(s: &str);

    #[wasm_bindgen(js_name = updateJsSelectedBonesList)]
    fn update_js_selected_bones_list(bones_json: String);

    #[wasm_bindgen(js_name = notifyJsEntitySelected)]
    fn notify_js_entity_selected(entity_id: u32);

    #[wasm_bindgen(js_name = populateImportAnimationDropdown)]
    fn populate_import_animation_dropdown(animations_json: String);

    // Bridge function to force-refresh the JS entity panel hierarchy
    #[wasm_bindgen(js_name = refreshJsEntityList)]
    fn refresh_js_entity_list();
}

// Global thread-safe pipeline variables tracking incoming models for background inspection
static ONGOING_INSPECTION_HANDLE: Mutex<Option<Handle<Gltf>>> = Mutex::new(None);
static INCOMING_SPAWN_QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn register_virtual_glb_asset(file_name: String, file_bytes: &[u8]) {
    if let Ok(mut queue) = INCOMING_ASSETS.lock() {
        queue.push((file_name, file_bytes.to_vec()));
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn spawn_imported_entity(entity_json: &str) {
    // Push the raw JSON string directly into your existing thread-safe spawn queue
    if let Ok(mut queue) = INCOMING_SPAWN_QUEUE.lock() {
        queue.push(entity_json.to_string());
    }
}
//supported commands
#[derive(Debug, Clone)]
pub enum WebCommandAction {
    //Just for hierercy panel stuff
    NudgeX(f32),
    NudgeY(f32),
    NudgeZ(f32),
    SetScaleX(f32),
    SetScaleY(f32),
    SetScaleZ(f32),
    SetRotationX(f32),
    SetRotationY(f32),
    SetRotationZ(f32),
    SetModelPath(String),
    SetMaterialColor([f32; 3]),
    CreateNewEntity(String), // New command to create an entity with a given name
}

#[derive(Debug, Clone)]
pub struct WebCommandPayload {
    pub entity_id: u32,
    pub action: WebCommandAction,
}

// Append or extend into your enum component options mapping
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum JsonComponentKind {
    Mesh {
        mesh_type: String,
    },
    Material {
        color_rgb: [f32; 3],
    },
    GltfModel {
        path: String,
    },
    ActiveAnimation {
        animation_name: String,
        looping: bool,
    },
    ExternalComponent {
        file_name: String,
    },
    ProceduralSky,
    ImageSkybox {
        path: String,
        brightness: f32,
    },
    AnimationRig {
        bone_groups: Vec<BoneGroupJson>,
    },
}

#[derive(Component)]
pub struct ModelAnimationConfiguration {
    pub animation_name: String,
    pub looping: bool,
}

#[derive(Resource, Default)]
pub struct JavaScriptRiggingCommandQueue {
    pub incoming_commands: Vec<(u32, bool)>, // (json_id, activate)
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
    pub is_local: bool,
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

#[derive(Component)]
pub struct GltfModelPathMarker {
    pub path: String,
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
pub struct BoneGroupJson {
    pub group_name: String,      // e.g., "UpperBody"
    pub bone_names: Vec<String>, // e.g., ["Spine", "Shoulder.L", "Shoulder.R"]
}

#[derive(Resource, Default)]
pub struct RiggingSetupState {
    pub is_active: bool,
    pub target_entity: Option<Entity>, // The main GLTF parent entity being rigged
    pub selected_bones: Vec<String>,   // Bone names currently highlighted by the user
}

#[derive(Asset, TypePath, Clone, Debug)]
pub struct SceneAsset {
    pub items: Vec<SceneJsonDeserialize>,
}

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
    let memory_reader = bevy::asset::io::memory::MemoryAssetReader::default();

    let virtual_dir_clone = memory_reader.root.clone();

    // Move the shared reader instance straight into the asset source lifecycle provider
    app.register_asset_source(
        "models",
        bevy::asset::io::AssetSourceBuilder::new(move || Box::new(memory_reader.clone())),
    );

    app.insert_resource(WasmAssetCache {
        models: std::collections::HashMap::new(),
        handles: Vec::new(),
        virtual_dir: Some(virtual_dir_clone),
    });

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
    app.init_resource::<RiggingSetupState>();
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
            process_js_rigging_commands_system,
            bone_selection_and_rendering_system,
            auto_initialize_gltf_default_pose_system,
            process_wasm_importer_queues_system,
            inspect_loading_glb_animations_system,
            apply_named_animations_from_json_system,
            process_wasm_dynamic_assets_system,
            process_js_nudge_commands_system,
        ),
    );
}

fn setup_editor_environment(
    mut commands: Commands,
    mut gizmo_config_store: ResMut<GizmoConfigStore>,
    asset_server: Res<AssetServer>,
) {
    for (_, config, _) in gizmo_config_store.iter_mut() {
        config.render_layers = GIZMO_LAYER;
    }

    let handle = asset_server.load::<SceneAsset>("scene.json");
    commands.insert_resource(CurrentSceneHandle(handle));

    commands
        .spawn((
            Camera3d::default(),
            bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
            Transform::from_xyz(0.0, 5.0, 10.0),
            RenderLayers::layer(0),
            EditorCamera, // Your custom marker component
        ))
        .with_children(|parent| {
            // Child Overlay Camera: inherits parent position automatically
            parent.spawn((
                Camera3d {
                    // Clearing to 0.0 forces gizmos to render right on top!
                    depth_load_op: bevy::camera::Camera3dDepthLoadOp::Clear(0.0),
                    ..default()
                },
                Camera {
                    // Don't clear the color buffer (keep what the main camera painted)
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    // Render strictly after the main camera
                    order: 1,
                    ..default()
                },
                // because Camera3d automatically requests it as a required component!
                Msaa::Off,
                GIZMO_LAYER,
            ));
        });

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
                                GltfModelPathMarker { path: path.clone() },
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

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn toggle_rigging_mode(entity_json_id: u32, activate: bool) {
    if let Ok(mut commands) = JS_RIGGING_COMMANDS.lock() {
        commands.push((entity_json_id, activate));
    }
}

pub fn process_js_rigging_commands_system(
    mut rigging_state: ResMut<RiggingSetupState>,
    targets_query: Query<(Entity, &SceneEntity)>, // Query all active scene targets
) {
    // 1. Lock and extract any waiting commands sent by JavaScript
    let mut commands_to_process = Vec::new();
    if let Ok(mut commands) = JS_RIGGING_COMMANDS.lock() {
        if !commands.is_empty() {
            commands_to_process = std::mem::take(&mut *commands);
        }
    }

    // 2. Loop through the commands
    for (json_id, activate) in commands_to_process {
        rigging_state.is_active = activate;

        if activate {
            rigging_state.selected_bones.clear();
            rigging_state.target_entity = None;

            // Find the live Bevy Entity matching the requested Javascript id layout
            let mut found_bevy_entity = None;
            for (entity, scene_entity) in targets_query.iter() {
                if scene_entity.json_id == json_id {
                    found_bevy_entity = Some(entity);
                    break;
                }
            }

            rigging_state.target_entity = found_bevy_entity;

            if found_bevy_entity.is_some() {
                info!(
                    "Successfully activated Bone Rigging Setup Mode for JSON ID: {}",
                    json_id
                );
            } else {
                warn!(
                    "Rigging Error: Failed to find a matching live Bevy Entity for JSON ID: {}",
                    json_id
                );
            }
        } else {
            // Turning setup mode off
            rigging_state.target_entity = None;
            rigging_state.selected_bones.clear();
            info!("Exited Rigging Setup Mode.");
        }
    }
}

fn bone_selection_and_rendering_system(
    mut rigging_state: ResMut<RiggingSetupState>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    // Query children hierarchies that have a Name component (GLTF joints use Name)
    children_query: Query<(Entity, &Name, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    if !rigging_state.is_active {
        return;
    }

    // 1. Draw visual indicators around ALL bones of the target entity so the user sees them
    if let Some(target) = rigging_state.target_entity {
        // (For brevity, you can recursively look through children or draw a small wire sphere at each bone transform location)

        for (entity, name, global_transform) in children_query.iter() {
            gizmos.sphere(
                global_transform.translation(),
                0.1,
                Color::srgb(0.9, 0.8, 0.1),
            );

            if rigging_state.selected_bones.contains(&name.to_string()) {
                gizmos.sphere(
                    global_transform.translation(),
                    0.15,
                    Color::srgb(0.1, 0.9, 0.1),
                );
            }
        }
    }

    // 2. Intercept Raycast for Selection
    if mouse_input.just_pressed(MouseButton::Left) {
        let Ok(window) = window_query.single() else {
            return;
        };
        let Ok((camera, camera_transform)) = camera_query.single() else {
            return;
        };
        let Some(cursor_pos) = window.cursor_position() else {
            return;
        };
        let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else {
            return;
        };

        let mut closest_bone: Option<(String, f32)> = None;

        for (_entity, name, global_transform) in children_query.iter() {
            let pos = global_transform.translation();
            let distance = ray.origin.distance(pos);
            let to_object = pos - ray.origin;
            let projection = to_object.dot(*ray.direction);

            if projection > 0.0 {
                let closest_point = ray.origin + *ray.direction * projection;
                if closest_point.distance(pos) < 0.4 {
                    // Ray hit threshold for bones
                    if closest_bone.is_none() || distance < closest_bone.as_ref().unwrap().1 {
                        closest_bone = Some((name.to_string(), distance));
                    }
                }
            }
        }

        if let Some((bone_name, _)) = closest_bone {
            // Check if holding Control for multi-select
            if keyboard_input.pressed(KeyCode::ControlLeft)
                || keyboard_input.pressed(KeyCode::ControlRight)
            {
                if let Some(index) = rigging_state
                    .selected_bones
                    .iter()
                    .position(|x| x == &bone_name)
                {
                    rigging_state.selected_bones.remove(index); // Deselect if already added
                } else {
                    rigging_state.selected_bones.push(bone_name);
                }
            } else {
                // Single select clears previous list
                rigging_state.selected_bones = vec![bone_name];
            }

            // Send updated list across the bridge to JS side
            #[cfg(target_arch = "wasm32")]
            {
                if let Ok(serialized) = serde_json::to_string(&rigging_state.selected_bones) {
                    update_js_selected_bones_list(serialized);
                }
            }
        }
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
    rigging_state: Res<RiggingSetupState>,
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
    if !rigging_state.is_active && selection.active_axis.is_none() {
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

            if let Some((entity, _)) = closest_hit {
                selection.selected = Some(entity);
                selection.mode = GizmoMode::None;

                if let Ok((_, scene_entity)) = transforms_query.get(entity) {
                    #[cfg(target_arch = "wasm32")]
                    notify_js_entity_selected(scene_entity.json_id);
                }
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
                                let sensitivity = 0.005;
                                let angle = delta_x * sensitivity;
                                let view_rotation =
                                    Quat::from_axis_angle(*camera_transform.forward(), angle);
                                transform.rotation = view_rotation * initial_rot;
                            }
                        }
                        GizmoMode::Scale => {
                            if let Some(initial_scale) = selection.backup_scale {
                                let sensitivity = 0.005;
                                let percentage = (1.0 + delta_x * sensitivity).max(0.05);
                                transform.scale = initial_scale * percentage;
                            }
                        }
                        _ => {}
                    }
                } else {
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
    let json_string = serde_json::to_string(&scene_data).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(local_storage)) = window.local_storage() {
                if let Ok(_) = local_storage.set_item("waddlie_current_scene", &json_string) {
                    js_console_log("Wasm Interop: Scene auto-saved to browser LocalStorage RAM.");

                    refresh_js_entity_list();
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("Desktop Auto-Save: {}", json_string);
    }
}

fn auto_initialize_gltf_default_pose_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    // Find animation players that haven't been configured with a graph yet
    mut player_query: Query<(Entity, &mut AnimationPlayer), Without<AnimationGraphHandle>>,
    // Look through parent structures to extract our custom path marker component
    parent_query: Query<&ChildOf>,
    path_marker_query: Query<&GltfModelPathMarker>,
) {
    for (entity, mut player) in player_query.iter_mut() {
        // Trace up the spawned child hierarchy to find the model's original string file path
        let mut current_ancestor = entity;
        let mut found_model_path: Option<String> = None;

        while let Ok(parent) = parent_query.get(current_ancestor) {
            current_ancestor = parent.get();
            if let Ok(marker) = path_marker_query.get(current_ancestor) {
                found_model_path = Some(marker.path.clone());
                break;
            }
        }

        // If we successfully found the model path, use it!
        // Otherwise, fall back gracefully to a common default like soldier or skip.
        let model_path = match found_model_path {
            Some(path) => path,
            None => continue, // Skip if it's not a known JSON/Imported GLTF hierarchy player
        };

        let gltf_animation_handle =
            asset_server.load(GltfAssetLabel::Animation(0).from_asset(model_path));

        let (graph, node_index) = AnimationGraph::from_clip(gltf_animation_handle);
        let graph_handle = graphs.add(graph);

        // Assign the graph to the entity
        commands
            .entity(entity)
            .insert(AnimationGraphHandle(graph_handle));

        let mut active_animation = player.play(node_index);
        active_animation.seek_to(0.0).pause();
    }
}

fn process_wasm_importer_queues_system(
    mut commands: Commands,
    mut scene_data: ResMut<LoadedSceneData>,
    asset_server: Res<AssetServer>,
) {
    let mut tasks = Vec::new();
    if let Ok(mut queue) = INCOMING_SPAWN_QUEUE.lock() {
        if !queue.is_empty() {
            tasks = std::mem::take(&mut *queue);
        }
    }

    for json_str in tasks {
        if let Ok(SceneJsonDeserialize::Entity {
            id,
            name,
            position_x,
            position_y,
            position_z,
            components,
            ..
        }) = serde_json::from_str::<SceneJsonDeserialize>(&json_str)
        {
            let entity_id = commands
                .spawn((
                    Name::new(name.clone()),
                    Transform::from_xyz(position_x, position_y, position_z),
                    Visibility::default(),
                    SceneEntity { json_id: id },
                ))
                .id();

            for comp in &components {
                match comp {
                    JsonComponentKind::GltfModel { path } => {
                        let gltf_scene_path = if path.starts_with("models://") {
                            format!("{}#Scene0", path)
                        } else {
                            format!("models://{}#Scene0", path)
                        };

                        // Passing string reference here is perfectly valid!
                        let child = commands
                            .spawn((
                                SceneRoot(asset_server.load(&gltf_scene_path)),
                                Transform::default(),
                                Visibility::default(),
                                GltfModelPathMarker { path: path.clone() },
                            ))
                            .id();
                        commands.entity(entity_id).add_child(child);

                        if let Ok(mut handle_store) = ONGOING_INSPECTION_HANDLE.lock() {
                            let gltf_container_path = if path.starts_with("models://") {
                                path.clone()
                            } else {
                                format!("models://{}", path)
                            };

                            let gltf_container_handle: Handle<Gltf> =
                                asset_server.load(&gltf_container_path);
                            *handle_store = Some(gltf_container_handle);
                        }
                    }
                    JsonComponentKind::ActiveAnimation {
                        animation_name,
                        looping,
                    } => {
                        commands
                            .entity(entity_id)
                            .insert(ModelAnimationConfiguration {
                                animation_name: animation_name.clone(),
                                looping: *looping,
                            });
                    }
                    _ => {}
                }
            }

            scene_data
                .items
                .push(serde_json::from_str(&json_str).unwrap());
            trigger_web_scene_download(&scene_data.items);
        }
    }
}

// Inspects the loading asset, and sends its animation name array to the UI dropdown
fn inspect_loading_glb_animations_system(gltf_assets: Res<Assets<Gltf>>) {
    let mut should_clear = false;
    if let Ok(handle_store) = ONGOING_INSPECTION_HANDLE.lock() {
        if let Some(handle) = &*handle_store {
            if let Some(gltf) = gltf_assets.get(handle) {
                // Collect string identifiers out of internal hash keys
                let mut names: Vec<String> = gltf
                    .named_animations
                    .keys()
                    .map(|k| k.to_string())
                    .collect();
                names.sort();

                #[cfg(target_arch = "wasm32")]
                {
                    if let Ok(serialized) = serde_json::to_string(&names) {
                        populate_import_animation_dropdown(serialized);
                    }
                }
                should_clear = true;
            }
        }
    }
    if should_clear {
        if let Ok(mut handle_store) = ONGOING_INSPECTION_HANDLE.lock() {
            *handle_store = None;
        }
    }
}

// Replaces the old index-based logic to look up string tracks out of the main GLTF handle
fn apply_named_animations_from_json_system(
    mut commands: Commands,
    gltf_assets: Res<Assets<Gltf>>,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut player_query: Query<(Entity, &mut AnimationPlayer), Without<AnimationGraphHandle>>,
    parent_query: Query<&ChildOf>,
    root_query: Query<&SceneRoot>,
    config_query: Query<&ModelAnimationConfiguration>,
) {
    for (player_entity, mut player) in player_query.iter_mut() {
        let mut current = player_entity;

        while let Ok(parent) = parent_query.get(current) {
            current = parent.get();

            // Check if the base root contains structural configuration tags
            if let Ok(config) = config_query.get(current) {
                // Determine source model path via lookups
                let gltf_handle: Handle<Gltf> = asset_server.load("models/soldier.glb"); // Extracted dynamically or mapped via configuration

                if let Some(gltf) = gltf_assets.get(&gltf_handle) {
                    // Resolve named track
                    if let Some(clip_handle) =
                        gltf.named_animations.get(config.animation_name.as_str())
                    {
                        let (graph, node_index) = AnimationGraph::from_clip(clip_handle.clone());
                        let graph_handle = graphs.add(graph);

                        commands
                            .entity(player_entity)
                            .insert(AnimationGraphHandle(graph_handle));
                        let mut active = player.play(node_index);
                        if config.looping {
                            active.repeat();
                        }

                        commands
                            .entity(current)
                            .remove::<ModelAnimationConfiguration>();
                        break;
                    }
                }
            }
        }
    }
}

pub fn process_wasm_dynamic_assets_system(
    mut commands: Commands,
    mut cache: ResMut<WasmAssetCache>,
    asset_server: Res<AssetServer>,
) {
    if let Ok(mut queue) = INCOMING_ASSETS.lock() {
        if !queue.is_empty() {
            for (file_name, bytes) in std::mem::take(&mut *queue) {
                // Ensure there are no lingering slash prefixes on the string key
                let clean_name = file_name.trim_start_matches('/');

                #[cfg(target_arch = "wasm32")]
                js_console_log(&format!(
                    "📥 [Rust Cache] Processing asset registration for: '{}'",
                    clean_name
                ));

                // Commit the asset to your virtual directory RAM
                if let Some(ref dir) = cache.virtual_dir {
                    // Create a path reference that matches what Bevy passes down to the reader
                    let asset_path = std::path::Path::new(clean_name);

                    // Insert the byte array under this exact path lookup key into Bevy's VFS
                    dir.insert_asset(asset_path, bytes);
                } else {
                    error!(
                        "❌ [Virtual DB] Error: virtual_dir structure reference missing from Resource!"
                    );
                    continue;
                }

                // Instruct the AssetServer to load this file out of your memory source
                // Append "#Scene0" so Bevy knows to extract the actual 3D scene out of the GLB container
                let path_string = format!("models://{}#Scene0", clean_name);

                // This gives it a 'static lifetime instead of borrowing from a local variable!
                let asset_path = bevy::asset::AssetPath::from(path_string);

                // Load the scene handle safely
                let scene_handle: Handle<Scene> = asset_server.load(asset_path);

                // Previusly we were making new entitiy for every model new we are seperating them
                //commands.spawn((
                //SceneRoot(scene_handle.clone()),
                //Transform::from_xyz(0.0, 0.0, 0.0),
                //));

                #[cfg(target_arch = "wasm32")]
                {
                    // Call the external JS updater bridge we declared earlier
                    refresh_js_entity_list();
                }

                // Store the untyped handle in your asset tracking cache so it isn't garbage collected
                cache.handles.push(scene_handle.untyped());
            }
        }
    }
}

//place that benvy is slave and js is master

//take the command from js and redirect
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn order_benvy(entity_id: u32, command: String, _value_str: String, value_num: f32) {
    // Match the incoming master command string to our typed variant structures
    let action = match command.as_str() {
        "nudge_x" => WebCommandAction::NudgeX(value_num),
        "nudge_y" => WebCommandAction::NudgeY(value_num),
        "nudge_z" => WebCommandAction::NudgeZ(value_num),
        "scale_x" => WebCommandAction::SetScaleX(value_num),
        "scale_y" => WebCommandAction::SetScaleY(value_num),
        "scale_z" => WebCommandAction::SetScaleZ(value_num),
        "rotation_x" => WebCommandAction::SetRotationX(value_num),
        "rotation_y" => WebCommandAction::SetRotationY(value_num),
        "rotation_z" => WebCommandAction::SetRotationZ(value_num),
        "model_path" => WebCommandAction::SetModelPath(_value_str),
        "material_color" => {
            // JavaScript sends individual channels or hex equivalents.
            // If passing uniform grayscale, this matches your signature.
            WebCommandAction::SetMaterialColor([value_num, value_num, value_num])
        }
        _ => {
            js_console_log(&format!("⚠️ Unknown command received from JS: {}", command));
            return;
        }
    };

    if let Ok(mut queue) = HIERARCHY_COMMANDS.lock() {
        queue.push(WebCommandPayload { entity_id, action });
    }
}

pub fn process_js_nudge_commands_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut transforms_query: Query<(&mut Transform, &SceneEntity, &Children)>,
    mut path_marker_query: Query<(Entity, &mut GltfModelPathMarker)>,
    mut scene_data: ResMut<LoadedSceneData>,
) {
    // 1. Drains multi-payload mutations safely out of the global mutex queue
    let mut tasks = Vec::new();
    if let Ok(mut queue) = HIERARCHY_COMMANDS.lock() {
        if !queue.is_empty() {
            tasks = std::mem::take(&mut *queue);
        }
    }

    if tasks.is_empty() {
        return;
    }

    let mut scene_updated = false;

    // 2. Parse and apply every command dynamically
    for command in tasks {
        for (mut transform, scene_entity, children) in transforms_query.iter_mut() {
            if scene_entity.json_id == command.entity_id {
                // Execute actions depending on matching variant rules
                match &command.action {
                    WebCommandAction::NudgeX(val) => transform.translation.x = *val,
                    WebCommandAction::NudgeY(val) => transform.translation.y = *val,
                    WebCommandAction::NudgeZ(val) => transform.translation.z = *val,

                    WebCommandAction::SetScaleX(val) => transform.scale.x = *val,
                    WebCommandAction::SetScaleY(val) => transform.scale.y = *val,
                    WebCommandAction::SetScaleZ(val) => transform.scale.z = *val,

                    WebCommandAction::SetRotationX(val) => {
                        let (_, y, z) = transform.rotation.to_euler(EulerRot::XYZ);
                        transform.rotation =
                            Quat::from_euler(EulerRot::XYZ, val.to_radians(), y, z);
                    }
                    WebCommandAction::SetRotationY(val) => {
                        let (x, _, z) = transform.rotation.to_euler(EulerRot::XYZ);
                        transform.rotation =
                            Quat::from_euler(EulerRot::XYZ, x, val.to_radians(), z);
                    }
                    WebCommandAction::SetRotationZ(val) => {
                        let (x, y, _) = transform.rotation.to_euler(EulerRot::XYZ);
                        transform.rotation =
                            Quat::from_euler(EulerRot::XYZ, x, y, val.to_radians());
                    }

                    WebCommandAction::SetModelPath(new_path) => {
                        for child in children.iter() {
                            if let Ok((child_entity, mut marker)) = path_marker_query.get_mut(child)
                            {
                                marker.path = new_path.clone();

                                let gltf_scene_path = if new_path.starts_with("models://") {
                                    format!("{}#Scene0", new_path)
                                } else {
                                    format!("models://{}#Scene0", new_path)
                                };

                                commands
                                    .entity(child_entity)
                                    .insert(SceneRoot(asset_server.load(&gltf_scene_path)));

                                #[cfg(target_arch = "wasm32")]
                                js_console_log(&format!(
                                    "🔄 Hot-swapped model hierarchy target to: {}",
                                    gltf_scene_path
                                ));
                                break;
                            }
                        }
                    }
                    WebCommandAction::SetMaterialColor(_val) => {}
                    WebCommandAction::CreateNewEntity(name) => {
                        commands.spawn((
                            Name::new(name.clone()),
                            Transform::default(),
                            Visibility::default(),
                            SceneEntity {
                                json_id: command.entity_id,
                            },
                        ));
                    }
                }

                // 3. Keep LoadedSceneData synced up accurately
                let (euler_x, euler_y, euler_z) = transform.rotation.to_euler(EulerRot::XYZ);

                for item in scene_data.items.iter_mut() {
                    if let SceneJsonDeserialize::Entity {
                        id,
                        position_x,
                        position_y,
                        position_z,
                        rotation_x,
                        rotation_y,
                        rotation_z,
                        scale_x,
                        scale_y,
                        scale_z,
                        color_rgb,
                        components,
                        ..
                    } = item
                    {
                        if *id == command.entity_id {
                            *position_x = transform.translation.x;
                            *position_y = transform.translation.y;
                            *position_z = transform.translation.z;
                            *rotation_x = euler_x.to_degrees();
                            *rotation_y = euler_y.to_degrees();
                            *rotation_z = euler_z.to_degrees();
                            *scale_x = transform.scale.x;
                            *scale_y = transform.scale.y;
                            *scale_z = transform.scale.z;

                            if let WebCommandAction::SetMaterialColor(rgb) = &command.action {
                                *color_rgb = *rgb;
                            }

                            if let WebCommandAction::SetModelPath(new_path) = &command.action {
                                for comp in components.iter_mut() {
                                    if let JsonComponentKind::GltfModel { path } = comp {
                                        *path = new_path.clone();
                                    }
                                }
                            }

                            scene_updated = true;
                            break;
                        }
                    }
                }

                #[cfg(target_arch = "wasm32")]
                js_console_log(&format!(
                    "🔧 Bevy executed operational task on entity ID: {}",
                    command.entity_id
                ));
                break;
            }
        }
    }

    // 4. Mirror everything back to local storage if mutations occurred
    if scene_updated {
        trigger_web_scene_download(&scene_data.items);
    }
}
