use crate::{
    Drift, Grounded, Landing, MainCamera, Momentum, Movement, OutsideForce, Player, PlayerAction,
};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use leafwing_input_manager::prelude::ActionState;

pub struct PlayerLocomotionPlugin;

impl Plugin for PlayerLocomotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(set_player_direction)
            .add_system(handle_player_acceleration.after(set_player_direction))
            .add_system(rotate_to_direction.after(set_player_direction))
            .add_system(apply_momentum.after(rotate_to_direction));
    }
}

const PLAYER_ROTATION_SPEED: f32 = 10.0;

#[derive(Resource)]
pub struct PlayerSpeed {
    accel_timer: Timer,
    base_speed: f32,
    current_speed: f32,
    top_speed: f32,
    min_speed: f32,
    acceleration: f32,
}

impl PlayerSpeed {
    pub fn reset(&mut self) {
        self.current_speed = self.base_speed;
        self.accel_timer.reset();
    }

    pub fn accelerate(&mut self, time: Res<Time>) {
        self.accel_timer.tick(time.delta());
        if self.accel_timer.finished() {
            if self.current_speed + 0.3 <= self.top_speed {
                self.current_speed = self.current_speed
                    + (self.top_speed - self.current_speed)
                        * (time.delta_seconds() * self.acceleration);
            } else {
                self.current_speed = self.top_speed;
            }
        }
    }

    pub fn current(&self) -> f32 {
        self.current_speed
    }
}

impl Default for PlayerSpeed {
    fn default() -> Self {
        PlayerSpeed {
            accel_timer: Timer::from_seconds(1.5, TimerMode::Once),
            base_speed: 7.5,
            current_speed: 7.5,
            top_speed: 15.0,
            min_speed: -20.0,
            acceleration: 2.0,
        }
    }
}

pub fn set_player_direction(
    mut player_query: Query<(&mut Movement, &Grounded, &ActionState<PlayerAction>), With<Player>>,
    camera_query: Query<&Transform, With<MainCamera>>,
) {
    let camera_transform = camera_query.single();
    let (mut movement, grounded, action) = player_query.single_mut();

    if grounded.is_grounded() {
        movement.0 = get_direction_in_camera_space(camera_transform, action);
    } else {
        if movement.is_moving() {
            movement.0 = Vec3::ZERO;
        }
    }
}

pub fn get_direction_in_camera_space(
    camera_transform: &Transform,
    action: &ActionState<PlayerAction>,
) -> Vec3 {
    let mut x = 0.0;
    let mut z = 0.0;

    let mut forward = camera_transform.forward();
    forward.y = 0.0;
    forward = forward.normalize();

    let mut right = camera_transform.right();
    right.y = 0.0;
    right = right.normalize();

    if action.pressed(PlayerAction::Up) {
        z += 1.0;
    }

    if action.pressed(PlayerAction::Down) {
        z -= 1.0;
    }

    if action.pressed(PlayerAction::Right) {
        x += 1.0;
    }

    if action.pressed(PlayerAction::Left) {
        x -= 1.0;
    }

    let right_vec: Vec3 = x * right;
    let forward_vec: Vec3 = z * forward;

    (right_vec + forward_vec).normalize_or_zero()
}

pub fn rotate_to_direction(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Movement, &Grounded, Option<&Landing>), With<Player>>,
    mut rotation_target: Local<Transform>,
) {
    let (mut transform, direction, grounded, is_landing) = query.single_mut();

    rotation_target.translation = transform.translation;
    let cur_position = rotation_target.translation;
    let flat_velo_direction = Vec3::new(direction.0.x, 0.0, direction.0.z).normalize_or_zero();
    if flat_velo_direction != Vec3::ZERO && grounded.is_grounded() {
        rotation_target.look_at(cur_position + flat_velo_direction, Vec3::Y);
        let turn_speed = if is_landing.is_some() {
            PLAYER_ROTATION_SPEED * 2.0
        } else {
            PLAYER_ROTATION_SPEED
        };
        transform.rotation = transform
            .rotation
            .slerp(rotation_target.rotation, time.delta_seconds() * turn_speed);
    }
}

pub fn handle_player_acceleration(
    time: Res<Time>,
    mut player_speed: ResMut<PlayerSpeed>,
    mut query: Query<(&mut Momentum, &Movement, &Grounded), With<Player>>,
) {
    let (mut momentum, movement, grounded) = query.single_mut();

    if movement.is_moving() {
        if grounded.is_grounded() {
            player_speed.accelerate(time);
            momentum.set(player_speed.current_speed);
        }
    } else {
        if grounded.is_grounded() {
            momentum.reset();
            player_speed.reset();
        }
    }
}

pub fn apply_momentum(
    mut query: Query<(
        &mut Velocity,
        &Transform,
        &Momentum,
        &Drift,
        Option<&OutsideForce>,
    )>,
) {
    let (mut velocity, transform, momentum, drift, has_force) = query.single_mut();

    let mut speed_to_apply = Vec3::ZERO;
    let mut should_change_velocity: bool = false;

    if let Some(outside_force) = has_force {
        should_change_velocity = true;
        speed_to_apply.x += outside_force.0.x;
        speed_to_apply.z += outside_force.0.z;
    }

    if momentum.has_momentum() {
        should_change_velocity = true;
        let forward = transform.forward();
        speed_to_apply += forward * momentum.get();
    }

    if drift.has_drift() {
        should_change_velocity = true;
        speed_to_apply += drift.0;
    }

    if should_change_velocity {
        velocity.linvel.x = speed_to_apply.x;
        velocity.linvel.z = speed_to_apply.z;
    }
}
