use crate::{
    DebugBall, Drift, Grounded, Landing, LedgeGrab, MainCamera, Momentum, Movement, OutsideForce,
    Player, PlayerAction,
};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use leafwing_input_manager::prelude::ActionState;

const PLAYER_ROTATION_SPEED: f32 = 10.0;

#[derive(Component)]
pub struct Crouching;

#[derive(Resource)]
pub struct PlayerSpeed {
    accel_timer: Timer,
    decel_timer: Timer,
    base_speed: f32,
    crawl_speed: f32,
    current_speed: f32,
    base_top_speed: f32,
    top_speed: f32,
    acceleration: f32,
    deceleration: f32,
}

impl PlayerSpeed {
    pub fn reset(&mut self) {
        self.current_speed = self.base_speed;
        self.top_speed = self.base_top_speed;
        self.accel_timer.reset();
        self.decel_timer.reset();
    }

    pub fn accelerate(&mut self, delta: std::time::Duration, seconds: f32) {
        self.accel_timer.tick(delta);
        if self.accel_timer.finished() {
            if self.current_speed + 0.3 <= self.top_speed {
                self.current_speed = self.current_speed
                    + (self.top_speed - self.current_speed) * (seconds * self.acceleration);
            } else {
                self.current_speed = self.top_speed;
            }
        }
    }

    pub fn decelerate(&mut self, delta: std::time::Duration, seconds: f32) {
        self.decel_timer.tick(delta);
        if self.decel_timer.finished() {
            if self.current_speed - 0.3 >= self.crawl_speed {
                self.current_speed = self.current_speed
                    + (self.crawl_speed - self.current_speed) * (seconds * self.deceleration);
            }
        }
    }

    pub fn current(&self) -> f32 {
        self.current_speed
    }

    pub fn set(&mut self, speed: f32) {
        self.top_speed = speed;
        self.current_speed = speed;
    }
}

impl Default for PlayerSpeed {
    fn default() -> Self {
        PlayerSpeed {
            accel_timer: Timer::from_seconds(0.3, TimerMode::Once),
            decel_timer: Timer::from_seconds(0.5, TimerMode::Once),
            base_speed: 7.5,
            crawl_speed: 4.0,
            current_speed: 7.5,
            top_speed: 15.0,
            base_top_speed: 15.0,
            acceleration: 1.0,
            deceleration: 2.0,
        }
    }
}

pub fn set_player_direction(
    mut player_query: Query<
        (&mut Movement, Option<&Grounded>, &ActionState<PlayerAction>),
        With<Player>,
    >,
    camera_query: Query<&Transform, With<MainCamera>>,
) {
    let camera_transform = camera_query.single();
    for (mut movement, grounded, action) in &mut player_query {
        if grounded.is_some() {
            movement.0 = get_direction_in_camera_space(camera_transform, action);
        } else {
            if movement.is_moving() {
                movement.0 = Vec3::ZERO;
            }
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

    if action.pressed(PlayerAction::Move) {
        let axis_pair = action.clamped_axis_pair(PlayerAction::Move).unwrap();
        x = axis_pair.x();
        z = axis_pair.y();
    }

    let right_vec: Vec3 = x * right;
    let forward_vec: Vec3 = z * forward;

    (right_vec + forward_vec).normalize_or_zero()
}

pub fn rotate_to_direction(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Movement, Option<&Landing>), (With<Player>, With<Grounded>)>,
    mut rotation_target: Local<Transform>,
) {
    for (mut transform, direction, is_landing) in &mut query {
        rotation_target.translation = transform.translation;
        let flat_velo_direction = Vec3::new(direction.0.x, 0.0, direction.0.z).normalize_or_zero();
        if flat_velo_direction != Vec3::ZERO {
            let target_position = rotation_target.translation + flat_velo_direction;

            rotation_target.look_at(target_position, Vec3::Y);
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
}

pub fn handle_player_speed(
    time: Res<Time>,
    mut player_speed: ResMut<PlayerSpeed>,
    mut query: Query<
        (&mut Momentum, &Movement, &ActionState<PlayerAction>),
        (With<Player>, With<Grounded>, Without<Crouching>),
    >,
) {
    for (mut momentum, movement, action) in &mut query {
        if movement.is_moving() {
            if action.pressed(PlayerAction::Crouch) {
                player_speed.decelerate(time.delta(), time.delta_seconds());
            } else {
                player_speed.accelerate(time.delta(), time.delta_seconds());
            }
            momentum.set(player_speed.current_speed);
        } else {
            momentum.reset();
            player_speed.reset();
        }
    }
}

pub fn apply_momentum(
    mut query: Query<
        (
            &mut Velocity,
            &Transform,
            &Momentum,
            &Drift,
            Option<&OutsideForce>,
        ),
        Without<LedgeGrab>,
    >,
) {
    for (mut velocity, transform, momentum, drift, has_force) in &mut query {
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
}
