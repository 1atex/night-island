use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    #[default]
    Idle,
    Walking,
    Sitting,
    Sleeping,
    Attentive,
    Held,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorEvent {
    pub previous: Activity,
    pub current: Activity,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BehaviorPose {
    pub walk: f32,
    pub stride: f32,
    pub facing: f32,
    pub sit: f32,
    pub sleep: f32,
}

pub struct Environment {
    pub dt: f32,
    pub enabled: bool,
    pub reduced_motion: bool,
    pub engaged: bool,
    pub held: bool,
    pub position: [f32; 2],
    pub bounds: [f32; 4],
    pub scale: f32,
}

pub struct BehaviorFrame {
    pub activity: Activity,
    pub position: Option<[f32; 2]>,
    pub pose: BehaviorPose,
    pub event: Option<BehaviorEvent>,
}

struct Walk {
    from: [f32; 2],
    to: [f32; 2],
    duration: f32,
    elapsed: f32,
}

pub struct PetBehavior {
    activity: Activity,
    remaining: f32,
    unattended: f32,
    bedtime: f32,
    walk: Option<Walk>,
    pose: BehaviorPose,
    rng: u32,
    bounds: Option<[f32; 4]>,
}

impl Default for PetBehavior {
    fn default() -> Self {
        Self::new(0x79b9_9e37)
    }
}

impl PetBehavior {
    pub fn new(seed: u32) -> Self {
        let mut pet = Self {
            activity: Activity::Idle,
            remaining: 6.0,
            unattended: 0.0,
            bedtime: 80.0,
            walk: None,
            pose: BehaviorPose::default(),
            rng: seed.max(1),
            bounds: None,
        };
        pet.bedtime = pet.random(65.0, 95.0);
        pet
    }

    fn random(&mut self, min: f32, max: f32) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        min + (max - min) * (self.rng as f64 / u32::MAX as f64) as f32
    }

    pub fn suspend(&mut self) {
        self.activity = Activity::Idle;
        self.walk = None;
        self.remaining = 6.0;
        self.unattended = 0.0;
        self.pose = BehaviorPose::default();
        self.bounds = None;
    }

    fn sit(&mut self) {
        self.activity = Activity::Sitting;
        self.remaining = self.random(8.0, 16.0);
        self.walk = None;
    }

    fn plan_walk(&mut self, env: &Environment) {
        let [left, top, right, bottom] = env.bounds;
        let x = env.position[0].clamp(left, right.max(left));
        let y = env.position[1].clamp(top, bottom.max(top));
        let distance = self.random(90.0, 220.0) * env.scale;
        let direction = if self.random(0.0, 1.0) < 0.5 { -1.0 } else { 1.0 };
        let mut target = (x + direction * distance).clamp(left, right.max(left));
        if (target - x).abs() < 36.0 * env.scale {
            target = (x - direction * distance).clamp(left, right.max(left));
        }
        if (target - x).abs() < 24.0 * env.scale {
            self.sit();
            return;
        }
        self.activity = Activity::Walking;
        self.walk = Some(Walk {
            from: [x, y],
            to: [target, y],
            duration: (target - x).abs() / (34.0 * env.scale) + 0.6,
            elapsed: 0.0,
        });
    }

    pub fn advance(&mut self, env: &Environment) -> BehaviorFrame {
        let previous = self.activity;
        let dt = if env.dt.is_finite() { env.dt.clamp(0.0, 0.25) } else { 0.0 };
        let mut position = None;
        let mut walking = 0.0;
        if !env.enabled {
            self.suspend();
        } else {
            if self.bounds.is_some_and(|bounds| bounds != env.bounds) {
                self.suspend();
            }
            self.bounds = Some(env.bounds);
            if env.held || env.engaged {
                self.activity = if env.held { Activity::Held } else { Activity::Attentive };
                self.walk = None;
                self.unattended = 0.0;
                self.remaining = 6.0;
            } else {
                if matches!(self.activity, Activity::Held | Activity::Attentive) {
                    self.activity = Activity::Idle;
                    self.remaining = self.random(5.0, 9.0);
                }
                self.unattended += dt;
                self.remaining -= dt;
                if env.reduced_motion && self.walk.is_some() {
                    self.sit();
                }
                if self.unattended >= self.bedtime && self.activity != Activity::Sleeping {
                    if self.activity != Activity::Sitting {
                        self.sit();
                        self.remaining = 5.0;
                    } else if self.remaining <= 0.0 {
                        self.activity = Activity::Sleeping;
                        self.remaining = self.random(30.0, 55.0);
                    }
                } else if self.activity == Activity::Walking {
                    if let Some(walk) = &mut self.walk {
                        walk.elapsed = (walk.elapsed + dt).min(walk.duration);
                        let u = walk.elapsed / walk.duration;
                        let ease = u * u * (3.0 - 2.0 * u);
                        let distance = walk.to[0] - walk.from[0];
                        position = Some([walk.from[0] + distance * ease, walk.from[1]]);
                        walking = (6.0 * u * (1.0 - u)).min(1.0);
                        self.pose.facing = distance.signum();
                        self.pose.stride = ((distance * ease).abs() / (22.0 * env.scale) * std::f32::consts::TAU)
                            % std::f32::consts::TAU;
                        if u >= 1.0 {
                            self.sit();
                        }
                    }
                } else if self.remaining <= 0.0 {
                    match self.activity {
                        Activity::Sleeping => {
                            self.unattended = 0.0;
                            self.bedtime = self.random(65.0, 95.0);
                            self.activity = Activity::Sitting;
                            self.remaining = 4.0;
                        }
                        Activity::Sitting => {
                            self.activity = Activity::Idle;
                            self.remaining = self.random(3.0, 6.0);
                        }
                        _ if !env.reduced_motion => self.plan_walk(env),
                        _ => self.sit(),
                    }
                }
            }
            let blend = if env.reduced_motion { 1.0 } else { 1.0 - (-dt * 7.0).exp() };
            let sitting = f32::from(matches!(self.activity, Activity::Sitting | Activity::Sleeping));
            let sleeping = f32::from(self.activity == Activity::Sleeping);
            self.pose.walk += (walking - self.pose.walk) * blend;
            self.pose.sit += (sitting - self.pose.sit) * blend;
            self.pose.sleep += (sleeping - self.pose.sleep) * blend;
            if self.pose.walk < 0.005 {
                self.pose.walk = 0.0;
            }
        }
        BehaviorFrame {
            activity: self.activity,
            position,
            pose: self.pose,
            event: (previous != self.activity).then_some(BehaviorEvent { previous, current: self.activity }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment() -> Environment {
        Environment {
            dt: 1.0 / 30.0,
            enabled: true,
            reduced_motion: false,
            engaged: false,
            held: false,
            position: [400.0, 200.0],
            bounds: [0.0, 0.0, 1832.0, 952.0],
            scale: 1.0,
        }
    }

    #[test]
    fn pets_walk_rest_sleep_and_wake_with_bounded_frame_independent_steps() {
        for fps in [10, 20, 30, 60] {
            let mut pet = PetBehavior::new(42);
            let mut env = environment();
            env.dt = 1.0 / fps as f32;
            let mut activities = Vec::new();
            for _ in 0..fps * 240 {
                let frame = pet.advance(&env);
                if let Some(position) = frame.position {
                    assert!((position[0] - env.position[0]).abs() <= 52.0 * env.dt + 0.01);
                    assert!(position[0] >= env.bounds[0] && position[0] <= env.bounds[2]);
                    assert_eq!(position[1], 200.0);
                    env.position = position;
                }
                if let Some(event) = frame.event {
                    assert_ne!(event.previous, event.current);
                    activities.push(event.current);
                }
            }
            for state in [Activity::Walking, Activity::Sitting, Activity::Sleeping, Activity::Idle] {
                assert!(activities.contains(&state), "{fps}: {state:?}");
            }
        }
    }

    #[test]
    fn interacting_disabling_and_reduced_motion_stop_travel_immediately() {
        let mut env = environment();
        let mut pet = PetBehavior::new(42);
        for _ in 0..240 {
            if let Some(position) = pet.advance(&env).position {
                env.position = position;
            }
        }
        assert_eq!(pet.activity, Activity::Walking);
        env.held = true;
        let frame = pet.advance(&env);
        assert_eq!(frame.activity, Activity::Held);
        assert!(frame.position.is_none());
        env.held = false;
        env.engaged = true;
        assert_eq!(pet.advance(&env).activity, Activity::Attentive);
        env.engaged = false;
        env.enabled = false;
        let frame = pet.advance(&env);
        assert_eq!(frame.activity, Activity::Idle);
        assert_eq!(frame.pose.sit, 0.0);
        assert!(frame.position.is_none());
        env.enabled = true;
        env.reduced_motion = true;
        for _ in 0..6000 {
            assert!(pet.advance(&env).position.is_none());
        }
        env.engaged = true;
        let frame = pet.advance(&env);
        assert_eq!(frame.activity, Activity::Attentive);
        assert_eq!(frame.pose.sleep, 0.0);
    }

    #[test]
    fn monitor_changes_suspension_and_small_work_areas_never_cause_a_jump() {
        let mut env = environment();
        let mut pet = PetBehavior::new(5);
        for _ in 0..240 {
            if let Some(position) = pet.advance(&env).position {
                env.position = position;
            }
        }
        env.bounds = [-1920.0, -300.0, -88.0, 700.0];
        env.position = [-400.0, 100.0];
        assert!(pet.advance(&env).position.is_none());
        pet.suspend();
        env.dt = 3600.0;
        assert!(pet.advance(&env).position.is_none());
        env.bounds = [0.0, 0.0, 10.0, 10.0];
        env.position = [5.0, 5.0];
        env.dt = 0.1;
        for _ in 0..2000 {
            assert!(pet.advance(&env).position.is_none());
        }
    }
}
