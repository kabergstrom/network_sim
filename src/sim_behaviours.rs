use crate::sim::{
    behaviour_data, AsymmetricSimulationState, DeterministicSimulation, Sample,
    ServerRateSimulation, SimSettings, SimulationBehaviour, SimulationState,
};
use amethyst::core::{
    math::{self, Vector2},
    Time,
};
use lazy_static::*;
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc, time::Duration};
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct SineWaveIntegrationState {
    pos: Vector2<f32>,
    velocity: Vector2<f32>,
}
impl SimulationBehaviour for SineWaveIntegrationState {
    fn new_state(&self, settings: &SimSettings) -> Box<dyn SimulationState> {
        Box::new(Self::initial(settings))
    }
}
impl fmt::Display for SineWaveIntegrationState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Sine Wave Integration")
    }
}
impl Default for SineWaveIntegrationState {
    fn default() -> Self {
        Self {
            pos: math::zero(),
            velocity: math::zero(),
        }
    }
}
impl DeterministicSimulation for SineWaveIntegrationState {
    fn update(&mut self, abs_time: Duration, delta_time: Duration) {
        self.pos += self.velocity * delta_time.as_secs_f32();
        self.velocity += Vector2::new(0., 1.)
            * (abs_time.as_secs_f32() * 20.).sin() as f32
            * 1000.
            * delta_time.as_secs_f32();
    }
    fn pos_sample(&self) -> Sample {
        Sample { pos: self.pos }
    }
    fn initial(_settings: &SimSettings) -> Self {
        Self {
            pos: math::zero(),
            velocity: Vector2::new(100., 0.),
        }
    }
}

impl AsymmetricSimulationState for SineWaveIntegrationState {
    type SyncType = Self;
    fn send_state(&mut self) -> &Self::SyncType {
        self
    }
    fn recv_state(&mut self, val: Self::SyncType) {
        *self = val;
    }
    fn update_render(&mut self, time: &Time) -> Option<Sample> {
        self.pos += self.velocity * time.delta_seconds();
        self.velocity += Vector2::new(0., 1.)
            * (time.absolute_time_seconds() * 20.).sin() as f32
            * 1000.
            * time.delta_seconds();
        Some(Sample { pos: self.pos })
    }
    fn update_server(&mut self, time: &Time) -> Sample {
        self.pos += self.velocity * time.delta_seconds();
        self.velocity += Vector2::new(0., 1.)
            * (time.absolute_time_seconds() * 20.).sin() as f32
            * 1000.
            * time.delta_seconds();
        Sample { pos: self.pos }
    }
}

impl splines::Interpolate<f32> for SineWaveIntegrationState {
    /// Linear interpolation.
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        Self {
            pos: <Vector2<f32> as splines::Interpolate<f32>>::lerp(a.pos, b.pos, t),
            velocity: a.velocity,
        }
    }

    fn cubic_hermite(
        _: (Self, f32),
        _: (Self, f32),
        _: (Self, f32),
        _: (Self, f32),
        _: f32,
    ) -> Self {
        unimplemented!()
    }

    /// Quadratic Bézier interpolation.
    fn quadratic_bezier(_: Self, _: Self, _: Self, _: f32) -> Self {
        unimplemented!()
    }

    /// Cubic Bézier interpolation.
    fn cubic_bezier(_: Self, _: Self, _: Self, _: Self, _: f32) -> Self {
        unimplemented!()
    }
}

lazy_static! {
    pub static ref SIM_BEHAVIOURS: Vec<(Arc<dyn SimulationBehaviour>, std::ffi::CString)> = vec![
        behaviour_data::<SineWaveIntegrationState>(),
        behaviour_data::<ServerRateSimulation<SineWaveIntegrationState>>(),
    ];
}
