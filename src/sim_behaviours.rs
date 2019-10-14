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
pub struct PosVel {
    pos: Vector2<f32>,
    velocity: Vector2<f32>,
}
impl Default for PosVel {
    fn default() -> Self {
        Self {
            pos: math::zero(),
            velocity: Vector2::new(100., 0.),
        }
    }
}
#[derive(Copy, Default, Clone, Debug)]
struct SineWaveClientSim {
    state: PosVel,
    start_time: Option<Duration>,
}
impl SimulationBehaviour for SineWaveClientSim {
    fn new_state(&self, _settings: &SimSettings) -> Box<dyn SimulationState> {
        Box::new(Self::default())
    }
}
impl fmt::Display for SineWaveClientSim {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Sine Wave Render-Rate Client Sim")
    }
}
impl AsymmetricSimulationState for SineWaveClientSim {
    type SyncType = PosVel;
    fn send_state(&self) -> &Self::SyncType {
        &self.state
    }
    fn recv_state(&mut self, val: Self::SyncType, time: &Time) {
        self.state = val;
        if let None = self.start_time {
            self.start_time = Some(time.absolute_time());
        }
    }
    fn update_render(&mut self, time: &Time) -> Option<Sample> {
        self.start_time.map(|t| {
            self.state.pos += self.state.velocity * time.delta_seconds();
            self.state.velocity += sine_wave(time.delta_time(), time.absolute_time() - t);
            Sample {
                pos: self.state.pos,
            }
        })
    }
    fn update_server(&mut self, time: &Time) -> Sample {
        self.state.pos += self.state.velocity * time.delta_seconds();
        self.state.velocity += sine_wave(time.delta_time(), time.absolute_time());
        Sample {
            pos: self.state.pos,
        }
    }
}
#[derive(Copy, Default, Serialize, Deserialize, Clone, Debug)]
struct SineWaveDeterministicSim {
    state: PosVel,
}
impl fmt::Display for SineWaveDeterministicSim {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Sine Wave Server-Rate Client Sim")
    }
}
impl DeterministicSimulation for SineWaveDeterministicSim {
    type SyncType = PosVel;
    fn send_state(&self) -> &Self::SyncType {
        &self.state
    }
    fn recv_state(&mut self, val: Self::SyncType) {
        self.state = val;
    }
    fn update(&mut self, abs_time: Duration, delta_time: Duration) {
        self.state.pos += self.state.velocity * delta_time.as_secs_f32();
        self.state.velocity += sine_wave(delta_time, abs_time);
    }
    fn pos_sample(&self, state: &Self::SyncType) -> Sample {
        Sample { pos: state.pos }
    }
    fn initial(_settings: &SimSettings) -> Self {
        Self::default()
    }
}

#[derive(Default)]
pub struct SineWaveThinClientCreator;
impl fmt::Display for SineWaveThinClientCreator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Thin Client Sine Wave")
    }
}
impl SimulationBehaviour for SineWaveThinClientCreator {
    fn new_state(&self, settings: &SimSettings) -> Box<dyn SimulationState> {
        Box::new(SineWaveThinClient {
            sim_state: Default::default(),
            sample_buffer: splines::Spline::from_vec(vec![]),
            delay: settings.render_interpolation_delay,
            start_time: None,
            recv_sample_server_time: false,
        })
    }
}

#[derive(Default)]
struct SineWaveThinClientServerTime;
impl fmt::Display for SineWaveThinClientServerTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Thin Client Sine Wave + Server Sample Correction")
    }
}
impl SimulationBehaviour for SineWaveThinClientServerTime {
    fn new_state(&self, settings: &SimSettings) -> Box<dyn SimulationState> {
        Box::new(SineWaveThinClient {
            sim_state: Default::default(),
            sample_buffer: splines::Spline::from_vec(vec![]),
            delay: settings.render_interpolation_delay,
            start_time: None,
            recv_sample_server_time: true,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SineWaveThinClient {
    sim_state: PosVel,
    sample_buffer: splines::Spline<f32, PosVel>,
    delay: f32,
    start_time: Option<f32>,
    recv_sample_server_time: bool,
}

impl SimulationState for SineWaveThinClient {
    fn send_sync(&self, _time: &Time) -> Vec<u8> {
        bincode::serialize(&self.sim_state).unwrap()
    }
    fn recv_sync(&mut self, time: &Time, server_time: Duration, _server_frame: u64, msg: &Vec<u8>) {
        let sample = bincode::deserialize(msg).unwrap();
        if let None = self.start_time {
            self.start_time = Some(time.absolute_time().as_secs_f32());
        }
        let time = if self.recv_sample_server_time {
            server_time.as_secs_f32()
        } else {
            time.absolute_time().as_secs_f32()
        };
        self.sample_buffer.add(splines::Key::new(
            time,
            sample,
            splines::Interpolation::Linear,
        ));
    }
    fn update_render(&mut self, time: &Time) -> Option<Sample> {
        self.start_time
            .and_then(|start_time| {
                let t = time.absolute_time().as_secs_f32() - (self.delay / 1000.);
                if t < start_time {
                    return None;
                }
                self.sample_buffer.clamped_sample(t)
            })
            .map(|p| Sample { pos: p.pos })
    }
    fn update_server(&mut self, time: &Time) -> Sample {
        self.sim_state.pos += self.sim_state.velocity * time.delta_seconds();
        self.sim_state.velocity += sine_wave(time.delta_time(), time.absolute_time());
        Sample {
            pos: self.sim_state.pos,
        }
    }
}
#[derive(Default)]
struct SineWavePureFunctionCreator;
impl fmt::Display for SineWavePureFunctionCreator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Sine Wave Pure Function")
    }
}
impl SimulationBehaviour for SineWavePureFunctionCreator {
    fn new_state(&self, _settings: &SimSettings) -> Box<dyn SimulationState> {
        Box::new(SineWavePureFunction {
            sim_state: Default::default(),
            start_time: None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SineWavePureFunction {
    sim_state: PosVel,
    start_time: Option<f32>,
}

impl SimulationState for SineWavePureFunction {
    fn send_sync(&self, time: &Time) -> Vec<u8> {
        bincode::serialize(&self.sim_state).unwrap()
    }
    fn recv_sync(
        &mut self,
        time: &Time,
        _server_time: Duration,
        _server_frame: u64,
        _msg: &Vec<u8>,
    ) {
        if let None = self.start_time {
            self.start_time = Some(time.absolute_time().as_secs_f32());
        }
    }
    fn update_render(&mut self, time: &Time) -> Option<Sample> {
        self.start_time.and_then(|start_time| {
            let t = time.absolute_time().as_secs_f32() - start_time;
            if t < 0. {
                return None;
            }
            Some(Sample {
                pos: sine_wave(Duration::from_secs_f32(1.), Duration::from_secs_f32(t))
                    + time.absolute_time_seconds() as f32 * Vector2::new(2000., 2000.),
            })
        })
    }
    fn update_server(&mut self, time: &Time) -> Sample {
        Sample {
            pos: sine_wave(Duration::from_secs_f32(1.), time.absolute_time())
                + time.absolute_time_seconds() as f32 * Vector2::new(2000., 2000.),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct InputPosVel {
    input_dir: Vector2<f32>,
    pos: Vector2<f32>,
    velocity: Vector2<f32>,
}
impl Default for InputPosVel {
    fn default() -> Self {
        Self {
            input_dir: math::zero(),
            pos: math::zero(),
            velocity: math::zero(),
        }
    }
}

#[derive(Debug, Default)]
pub struct PlayerCharacterDeterministic {
    state: InputPosVel,
    server: bool,
}
impl Clone for PlayerCharacterDeterministic {
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            server: self.server,
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.state = source.state;
    }
}
impl fmt::Display for PlayerCharacterDeterministic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Player Character Server-Rate")
    }
}

impl DeterministicSimulation for PlayerCharacterDeterministic {
    type SyncType = InputPosVel;
    fn send_state(&self) -> &Self::SyncType {
        &self.state
    }
    fn recv_state(&mut self, val: Self::SyncType) {
        self.state = val;
    }
    fn update(&mut self, abs_time: Duration, delta_time: Duration) {
        if self.server {
            self.state.input_dir = PLAYER_INPUT_DIR
                .clamped_sample(abs_time.as_secs_f32())
                .unwrap();
        }
        self.state.velocity = self.state.input_dir * 100.;
        self.state.pos += self.state.velocity * delta_time.as_secs_f32();
    }
    fn pos_sample(&self, state: &Self::SyncType) -> Sample {
        Sample { pos: state.pos }
    }
    fn initial(_settings: &SimSettings) -> Self {
        Self {
            server: true,
            ..Default::default()
        }
    }
}

fn sine_wave(delta_time: Duration, abs_time: Duration) -> Vector2<f32> {
    Vector2::new(0., 1.)
        * (abs_time.as_secs_f32() * 20.).sin()
        * 300 as f32
        * delta_time.as_secs_f32()
}

macro_rules! spline_key {
    ( $time: expr => $x: expr , $y: expr ) => {{
        splines::Key::new($time, Vector2::new($x, $y), splines::Interpolation::Linear)
    }};
}

lazy_static! {
    pub static ref PLAYER_INPUT_DIR: splines::Spline<f32, Vector2<f32>> =
        splines::Spline::from_vec(vec![
            spline_key!(0. => 0., 0.),
            spline_key!(0.3 => 1., 0.),
            spline_key!(0.5 => 0., 0.),
            spline_key!(0.7 => 0., 1.),
            spline_key!(1. => 0., 0.),
            spline_key!(1.5 => -1., 0.),
            spline_key!(2.0 => 0., 0.),
        ]);
}

lazy_static! {
    pub static ref SIM_BEHAVIOURS: Vec<(Arc<dyn SimulationBehaviour>, std::ffi::CString)> = vec![
        behaviour_data::<SineWaveClientSim>(),
        behaviour_data::<ServerRateSimulation<SineWaveDeterministicSim>>(),
        behaviour_data::<SineWaveThinClientCreator>(),
        behaviour_data::<SineWaveThinClientServerTime>(),
        behaviour_data::<SineWavePureFunctionCreator>(),
        behaviour_data::<ServerRateSimulation<PlayerCharacterDeterministic>>(),
    ];
}

impl splines::Interpolate<f32> for InputPosVel {
    /// Linear interpolation.
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        Self {
            input_dir: a.input_dir,
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

impl splines::Interpolate<f32> for PosVel {
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
