use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::gizmos::config::GizmoConfigStore;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{Atmosphere, AtmospherePlugin, ScatteringMedium};
use bevy::prelude::*;
use bevy::state::commands;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Write, read_to_string};

// In your EditorSelection struct resource, change mode:

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
    pub initial_drag_value: Option<Vec3>, // Stores position/rotation/scale when drag started
    pub last_intersect_point: Option<Vec3>,
    pub backup_translation: Option<Vec3>,
    pub backup_rotation: Option<Quat>,
    pub backup_scale: Option<Vec3>,
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
pub struct CurrentScenePath(pub String);

#[derive(Resource, Default)]
pub struct LoadedSceneData {
    pub items: Vec<SceneJsonDeserialize>,
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

pub fn boot_editor_base(app: &mut App) {
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Waddlie Engine".to_string(),
            ..default()
        }),
        ..default()
    }));

    //app.add_plugins(AtmospherePlugin);

    app.insert_resource(CurrentScenePath("assets/scene.json".to_string()));
    app.init_resource::<EditorSelection>();
    app.init_resource::<LoadedSceneData>();
    app.insert_resource(ClearColor(Color::srgba(0.5, 0.7, 0.9, 1.0)));

    app.add_systems(
        Startup,
        (setup_editor_enviroment, load_and_construct_editor_scene),
    );
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

fn setup_editor_enviroment(
    mut commands: Commands,
    mut gizmo_config_store: ResMut<GizmoConfigStore>,
) {
    if let Some((_, config, _)) = gizmo_config_store.iter_mut().next() {
        config.depth_bias = -1.0;
    }
    // Spawn Stuff

    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(0.0, 0.0, 0.0),
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
        brightness: 200.0, //lumens i think
        ..default()
    });
}

//here lets contsruct our scene from json!!

fn load_and_construct_editor_scene(
    mut commands: Commands,
    scene_path: Res<CurrentScenePath>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materails: ResMut<Assets<StandardMaterial>>,
    mut scaterring_mediums: ResMut<Assets<ScatteringMedium>>,
    asset_server: Res<AssetServer>,
    mut scene_data_res: ResMut<LoadedSceneData>,
) {
    //lets get the items from our json file
    let Ok(file_content) = std::fs::read_to_string(&scene_path.0) else {
        warn!("Could not find any scene file at{}", &scene_path.0);
        return;
    };

    let world: Vec<SceneJsonDeserialize> = serde_json::from_str(&file_content).unwrap_or_default();
    scene_data_res.items = world.clone();

    let mut id_to_entity_map: HashMap<u32, Entity> = HashMap::new();
    let mut parent_child_relations: Vec<(Entity, u32)> = Vec::new();

    //lets build the scen
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
                        let mat_handle = materails.add(StandardMaterial {
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
                        let medium_handle = scaterring_mediums.add(ScatteringMedium::default());
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
}

/// System to handle 1, 2, 3 mode switching
fn gizmo_mode_switch_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<EditorSelection>,
) {
    if keyboard_input.just_pressed(KeyCode::Digit1) {
        selection.mode = GizmoMode::Translate;
        info!("Gizmo Mode: Translation (Move)");
    } else if keyboard_input.just_pressed(KeyCode::Digit2) {
        selection.mode = GizmoMode::Rotate;
        info!("Gizmo Mode: Rotation");
    } else if keyboard_input.just_pressed(KeyCode::Digit3) {
        selection.mode = GizmoMode::Scale;
        info!("Gizmo Mode: Scale");
    }
}

/// Advanced raycasting and plane-intersection manipulation engine
fn editor_gizmo_interaction_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    targets_query: Query<(Entity, &GlobalTransform), With<SceneEntity>>,
    mut transforms_query: Query<(&mut Transform, &SceneEntity)>,
    mut selection: ResMut<EditorSelection>,
    scene_path: Res<CurrentScenePath>,
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

    // --- STEP 1: OBJECT SELECTION FALLBACK (When completely idle) ---
    // --- STEP 1: OBJECT SELECTION FALLBACK (When not actively dragging/transforming) ---
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

            // 🔥 FIX: Successfully assign selection and reset the interaction mode state cleanly
            if closest_hit.is_some() {
                selection.selected = closest_hit.map(|(e, _)| e);
                selection.mode = GizmoMode::None;
            } else {
                selection.selected = None;
                selection.mode = GizmoMode::None;
            }
        }
    }

    // --- STEP 2: BLENDER MODAL TRIGGER HOTKEYS ---
    if let Some(selected_entity) = selection.selected {
        if selection.active_axis.is_none() {
            if let Ok((transform, _)) = transforms_query.get(selected_entity) {
                let mut chosen_mode = GizmoMode::None;

                if keyboard_input.just_pressed(KeyCode::KeyG) {
                    chosen_mode = GizmoMode::Translate;
                } else if keyboard_input.just_pressed(KeyCode::KeyR) {
                    chosen_mode = GizmoMode::Rotate;
                } else if keyboard_input.just_pressed(KeyCode::KeyS) {
                    chosen_mode = GizmoMode::Scale;
                }

                if chosen_mode != GizmoMode::None {
                    selection.mode = chosen_mode;

                    // Lock onto a camera-facing plane baseline when starting transformation
                    let plane_normal = camera_transform.forward();
                    let denom = ray.direction.dot(*plane_normal);
                    let initial_proj = if denom.abs() > 1e-5 {
                        let t = (transform.translation - ray.origin).dot(*plane_normal) / denom;
                        ray.origin + *ray.direction * t
                    } else {
                        transform.translation
                    };

                    selection.last_intersect_point = Some(initial_proj);
                    selection.initial_drag_value = Some(transform.translation);

                    // Save restoration state to enable Right-Click / Escape cancellation maps
                    selection.backup_translation = Some(transform.translation);
                    selection.backup_rotation = Some(transform.rotation);
                    selection.backup_scale = Some(transform.scale);

                    // Default to unconstrained viewport space dragging until X, Y, or Z is clicked
                    selection.active_axis = Some(SelectedAxis::None);
                }
            }
        }
    }

    // --- STEP 3: BLENDER AXIS LOCKING CONTROLS ---
    if selection.active_axis.is_some() {
        if keyboard_input.just_pressed(KeyCode::KeyX) {
            selection.active_axis = Some(SelectedAxis::X);
        } else if keyboard_input.just_pressed(KeyCode::KeyY) {
            selection.active_axis = Some(SelectedAxis::Y);
        } else if keyboard_input.just_pressed(KeyCode::KeyZ) {
            selection.active_axis = Some(SelectedAxis::Z);
        }
    }

    // --- STEP 4: ACTIVE LIVE TRANSFORMATION PROCESSING ---
    if selection.active_axis.is_some() {
        if let Some(selected_entity) = selection.selected {
            if let Ok((mut transform, scene_entity)) = transforms_query.get_mut(selected_entity) {
                let origin_ref = selection
                    .initial_drag_value
                    .unwrap_or(transform.translation);
                let active_axis = selection.active_axis.unwrap();

                // Compute plane intersections depending on chosen axis lock constraint rules
                let plane_normal = match active_axis {
                    SelectedAxis::X => Vec3::Y,
                    SelectedAxis::Y => Vec3::X,
                    SelectedAxis::Z => Vec3::Y,
                    SelectedAxis::None => *camera_transform.forward(),
                };

                let denom = ray.direction.dot(plane_normal);
                if denom.abs() > 1e-5 {
                    let t = (origin_ref - ray.origin).dot(plane_normal);
                    if t > 0.0 {
                        let current_intersect = ray.origin + *ray.direction * t;
                        let initial_intersect =
                            selection.last_intersect_point.unwrap_or(origin_ref);
                        let total_delta = current_intersect - initial_intersect;

                        match selection.mode {
                            GizmoMode::Translate => {
                                if let Some(initial_pos) = selection.backup_translation {
                                    match active_axis {
                                        SelectedAxis::X => {
                                            transform.translation =
                                                initial_pos + Vec3::new(total_delta.x, 0.0, 0.0)
                                        }
                                        SelectedAxis::Y => {
                                            transform.translation =
                                                initial_pos + Vec3::new(0.0, total_delta.y, 0.0)
                                        }
                                        SelectedAxis::Z => {
                                            transform.translation =
                                                initial_pos + Vec3::new(0.0, 0.0, total_delta.z)
                                        }
                                        SelectedAxis::None => {
                                            transform.translation = initial_pos + total_delta
                                        }
                                    }
                                }
                            }
                            GizmoMode::Rotate => {
                                if let Some(initial_rot) = selection.backup_rotation {
                                    let distance_delta =
                                        total_delta.length() * total_delta.x.signum() * 0.3;
                                    match active_axis {
                                        SelectedAxis::X => {
                                            transform.rotation =
                                                initial_rot * Quat::from_rotation_x(distance_delta)
                                        }
                                        SelectedAxis::Y => {
                                            transform.rotation =
                                                initial_rot * Quat::from_rotation_y(distance_delta)
                                        }
                                        SelectedAxis::Z => {
                                            transform.rotation =
                                                initial_rot * Quat::from_rotation_z(distance_delta)
                                        }
                                        SelectedAxis::None => {
                                            transform.rotation =
                                                initial_rot * Quat::from_rotation_z(distance_delta)
                                        }
                                    }
                                }
                            }
                            GizmoMode::Scale => {
                                if let Some(initial_scale) = selection.backup_scale {
                                    let scale_multiplier: f32 = 1.0 + (total_delta.x * 0.3);
                                    let factor = scale_multiplier.max(0.01_f32);
                                    match active_axis {
                                        SelectedAxis::X => {
                                            transform.scale = Vec3::new(
                                                initial_scale.x * factor,
                                                initial_scale.y,
                                                initial_scale.z,
                                            )
                                        }
                                        SelectedAxis::Y => {
                                            transform.scale = Vec3::new(
                                                initial_scale.x,
                                                initial_scale.y * factor,
                                                initial_scale.z,
                                            )
                                        }
                                        SelectedAxis::Z => {
                                            transform.scale = Vec3::new(
                                                initial_scale.x,
                                                initial_scale.y,
                                                initial_scale.z * factor,
                                            )
                                        }
                                        SelectedAxis::None => {
                                            transform.scale = initial_scale * factor
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // --- CONFIRMATION (Left Click) ---
                if mouse_input.just_pressed(MouseButton::Left) {
                    selection.active_axis = None;
                    selection.mode = GizmoMode::None; // Safe reset back to plain enum variant

                    // 🔥 FIX: Use the existing `transform` and `scene_entity` values instead of re-borrowing transforms_query
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
                    let _ = save_scene_to_disk(&scene_path.0, &scene_data.items);
                }

                // --- ESCAPE / CANCEL ACTION (Right Click or Esc Key) ---
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

            // Set up clean base axis color definitions
            let color_x = Color::srgb(1.0, 0.1, 0.1);
            let color_y = Color::srgb(0.1, 1.0, 0.1);
            let color_z = Color::srgb(0.1, 0.1, 1.0);

            // If an active transformation mode is engaged, project long infinitely tracking guideline indicators
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
                        // Highlight all 3 lines subtly if moving globally in screen space coordinates
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
                // Default view: Display clean 3D handle profiles while an item is highlighted idling
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

pub fn save_scene_to_disk(
    path: &str,
    scene_data: &Vec<SceneJsonDeserialize>,
) -> std::io::Result<()> {
    let json_string = serde_json::to_string_pretty(scene_data).unwrap();
    let mut file = File::create(path)?;
    file.write_all(json_string.as_bytes())?;
    Ok(())
}
