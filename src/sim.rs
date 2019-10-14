use std::time::Duration;

use amethyst::{
    core::{
        math::{self, Vector2},
        SystemDesc, Time,
    },
    ecs::{Read, ReadExpect, System, World, Write, WriteExpect},
    network::simulation::{
        memory::{channel as memory_channel, MemoryNetworkBundle},
        NetworkSimulationEvent, NetworkSimulationTime, TransportResource,
    },
    prelude::*,
    shrev::{EventChannel, ReaderId},
    utils::application_root_dir,
    Result,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Debug},
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub enum SimSide {
    Client,
    Server,
}
#[derive(Debug)]
pub struct WorldFrame<M: Debug + Clone> {
    pub side: SimSide,
    pub render_time: f32,
    pub net_time: f32,
    pub sample: M,
}
#[derive(Clone)]
pub struct SimSettings {
    pub curr_time: f32,
    pub sim_time_scale: f32,
    pub server_fps: u32,
    pub sync_rate: u32,
    pub render_fps: u32,
    pub render_time_variance: f32,
    pub duration: f32,
    pub render_interpolation_delay: f32,
    pub min_latency: f32,
    pub max_latency: f32,
    pub loss_percentage: f32,
    pub playing: bool,
    pub behaviour: Arc<dyn SimulationBehaviour>,
}
impl Default for SimSettings {
    fn default() -> Self {
        Self {
            curr_time: 0.0,
            sim_time_scale: 1.0,
            render_fps: 60,
            sync_rate: 30,
            server_fps: 30,
            duration: 0.5,
            render_interpolation_delay: 0.,
            render_time_variance: 0.,
            min_latency: 0.,
            max_latency: 0.,
            loss_percentage: 0.,
            playing: false,
            behaviour: Arc::new(crate::sim_behaviours::SineWaveThinClientCreator::default()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LocalClock {
    /// Clock offset's second part for constructing a Duration
    clock_offset_secs: i64,
    /// Clock offset's sub-second part for constructing a Duration
    clock_offset_nanos: i32,
    /// Time elapsed since the last frame.
    delta_time: Duration,
    /// The total number of frames that have been played in this session.
    frame_number: u64,
    /// The number of frames that have been played since last tick.
    frames_since_tick: u64,
    /// Time elapsed since game start, taking the speed multiplier into account.
    absolute_time: Duration,
    /// Time multiplier. Affects returned delta_seconds, delta_time and absolute_time.
    time_scale: Option<f32>,
    /// Duration per frame tick
    time_per_frame: Option<Duration>,
    /// Interpolation alpha-variable
    interpolation_alpha: f32,
}
impl Default for LocalClock {
    fn default() -> Self {
        Self {
            clock_offset_secs: 0,
            clock_offset_nanos: 0,
            delta_time: Duration::default(),
            frame_number: 0,
            frames_since_tick: 0,
            absolute_time: Duration::default(),
            time_scale: None,
            time_per_frame: None,
            interpolation_alpha: 0.,
        }
    }
}

impl LocalClock {
    fn new(
        offset_secs: i64,
        offset_nanos: i32,
        time_scale: Option<f32>,
        time_per_frame: Option<Duration>,
    ) -> Self {
        Self {
            clock_offset_secs: offset_secs,
            clock_offset_nanos: offset_nanos,
            time_scale,
            time_per_frame,
            ..Default::default()
        }
    }
    fn tick(&mut self, time: &Time) {
        let abs_time = if self.clock_offset_secs < 0 || self.clock_offset_nanos < 0 {
            time.absolute_time().checked_sub(Duration::new(
                (-self.clock_offset_secs) as u64,
                (-self.clock_offset_nanos) as u32,
            ))
        } else {
            Some(
                time.absolute_time()
                    + Duration::new(
                        self.clock_offset_secs as u64,
                        self.clock_offset_nanos as u32,
                    ),
            )
        };
        // only tick if the abs_time didn't wrap negative, i.e. we are "before the start"
        if let Some(abs_time) = abs_time {
            // Underflow here is misuse of the API, since Time should always
            let mut time_since_last = abs_time
                .checked_sub(self.absolute_time)
                .expect("Time is before LocalClock time");
            if let Some(time_scale) = self.time_scale {
                time_since_last = time_since_last.mul_f32(time_scale);
            }
            if let Some(time_per_frame) = self.time_per_frame {
                self.frames_since_tick = 0;

                while time_per_frame <= time_since_last {
                    time_since_last -= time_per_frame;
                    self.frames_since_tick += 1;
                    self.frame_number += 1;
                    self.absolute_time += time_per_frame;
                }
                self.delta_time = time_per_frame;
                if self.frame_number > 0 {
                    self.interpolation_alpha =
                        (time_since_last.as_secs_f64() / time_per_frame.as_secs_f64()) as f32;
                }
            } else {
                self.delta_time = self
                    .time_scale
                    .map(|scale| time.delta_time().mul_f32(scale))
                    .unwrap_or(time.delta_time());
                self.absolute_time = self
                    .time_scale
                    .map(|_| self.absolute_time + time_since_last)
                    .unwrap_or(abs_time); // if we don't have a time scale, we just use the abs_time directly
                self.frames_since_tick = 1;
                self.frame_number += 1;
                self.interpolation_alpha = 0.;
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ServerMessage {
    // contents of Duration
    server_secs: u64,
    server_nanos: u32,
    server_frame: u64,
    msg: Vec<u8>,
}

pub trait SimulationBehaviour: fmt::Display + Send + Sync + std::any::Any {
    fn new_state(&self, settings: &SimSettings) -> Box<dyn SimulationState>;
}
pub trait AsymmetricSimulationState {
    type SyncType: Serialize + for<'de> Deserialize<'de>;
    fn update_server(&mut self, time: &Time) -> Sample;
    fn send_state(&self) -> &Self::SyncType;
    fn recv_state(&mut self, val: Self::SyncType, time: &Time);
    fn send_sync(&self, _time: &Time) -> Vec<u8> {
        bincode::serialize(&self.send_state()).unwrap()
    }
    fn recv_sync(
        &mut self,
        time: &Time,
        _server_time: Duration,
        _server_frame: u64,
        msg: &Vec<u8>,
    ) {
        self.recv_state(bincode::deserialize(msg).unwrap(), time);
    }
    fn update_render(&mut self, time: &Time) -> Option<Sample>;
}
impl<T: AsymmetricSimulationState + Send + Sync + 'static> SimulationState for T {
    fn update_server(&mut self, time: &Time) -> Sample {
        <Self as AsymmetricSimulationState>::update_server(self, time)
    }
    fn send_sync(&self, time: &Time) -> Vec<u8> {
        <Self as AsymmetricSimulationState>::send_sync(self, time)
    }
    fn recv_sync(&mut self, time: &Time, server_time: Duration, server_frame: u64, msg: &Vec<u8>) {
        <Self as AsymmetricSimulationState>::recv_sync(self, time, server_time, server_frame, msg)
    }
    fn update_render(&mut self, time: &Time) -> Option<Sample> {
        <Self as AsymmetricSimulationState>::update_render(self, time)
    }
}

pub trait SimulationState: Send + Sync + std::any::Any {
    fn update_server(&mut self, time: &Time) -> Sample;
    fn send_sync(&self, time: &Time) -> Vec<u8>;
    fn recv_sync(&mut self, time: &Time, server_time: Duration, server_frame: u64, msg: &Vec<u8>);
    fn update_render(&mut self, time: &Time) -> Option<Sample>;
}

#[derive(Default)]
pub struct ServerRateSimulation<T> {
    _marker: std::marker::PhantomData<T>,
}
impl<T: fmt::Display + Default> fmt::Display for ServerRateSimulation<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", T::default())
    }
}
impl<T: DeterministicSimulation + fmt::Display> SimulationBehaviour for ServerRateSimulation<T> {
    fn new_state(&self, settings: &SimSettings) -> Box<dyn SimulationState> {
        Box::new(ServerRateSimulationState {
            clock: None,
            interpolation_buffer: splines::Spline::from_vec(vec![]),
            server_fps: settings.server_fps,
            prev_pos: math::zero(),
            client_sim: T::default(),
            last_server_frame: None,
            render_delay: settings.render_interpolation_delay,
            server: T::initial(settings),
        })
    }
}
#[derive(Clone)]
pub struct ServerRateSimulationState<T: DeterministicSimulation> {
    interpolation_buffer: splines::Spline<f32, T::SyncType>,
    prev_pos: Vector2<f32>,
    clock: Option<LocalClock>,
    client_sim: T,
    server: T,
    last_server_frame: Option<u64>,
    render_delay: f32,
    server_fps: u32,
}
impl<T: DeterministicSimulation> SimulationState for ServerRateSimulationState<T> {
    fn send_sync(&self, _time: &Time) -> Vec<u8> {
        bincode::serialize(self.server.send_state()).unwrap()
    }
    fn recv_sync(&mut self, time: &Time, server_time: Duration, server_frame: u64, msg: &Vec<u8>) {
        // start a new local clock that started server_time in the past
        if let None = self.clock {
            self.server.recv_state(bincode::deserialize(msg).unwrap());
            let diff = time.absolute_time() - server_time;
            let offset_secs = -(diff.as_secs() as i64);
            let offset_nanos = -(diff.as_nanos() as i32);
            let mut clock = LocalClock::new(
                offset_secs,
                offset_nanos,
                None,
                Some(Duration::from_secs_f32(1 as f32 / self.server_fps as f32)),
            );
            clock.frame_number = server_frame;
            clock.absolute_time = server_time;
            // add the first keyframe for the simulation
            let t = clock.absolute_time.as_secs_f32();
            self.interpolation_buffer.add(splines::Key::new(
                t,
                self.server.send_state().clone(),
                splines::Interpolation::Linear,
            ));
            self.clock = Some(clock);
            self.client_sim.clone_from(&self.server);
        } else if let Some(clock) = self.clock.as_mut() {
            // check if the incoming packet happened after our last received packet
            let newer_snapshot = self
                .last_server_frame
                .map(|f| f < server_frame)
                .unwrap_or(true);

            if newer_snapshot {
                if server_frame < clock.frame_number {
                    self.last_server_frame = None;
                    clock.frame_number = server_frame;
                    clock.absolute_time = server_time;
                    self.client_sim
                        .recv_state(bincode::deserialize(msg).unwrap());
                    for i in (0..self.interpolation_buffer.len()).rev() {
                        if self
                            .interpolation_buffer
                            .get(i)
                            .map(|k| k.t >= clock.absolute_time.as_secs_f32())
                            .unwrap_or(false)
                        {
                            self.interpolation_buffer.remove(i);
                        }
                    }
                } else {
                    self.last_server_frame = Some(server_frame);
                    self.server.recv_state(bincode::deserialize(msg).unwrap());
                }
            } else {
                // ignore reordered message
            }
        }
    }
    fn update_render(&mut self, time: &Time) -> Option<Sample> {
        if let Some(clock) = self.clock.as_mut() {
            clock.tick(time);
            for i in 1..=clock.frames_since_tick {
                let frame_time = clock
                    .time_per_frame
                    .unwrap()
                    .mul_f32((clock.frame_number - (clock.frames_since_tick - i)) as f32);
                // if this frame is the frame of our buffered server sample, just use the sample since
                // this frame's authoritative simulation result has already been calculated.
                // Otherwise perform a client-side simulation update
                if self
                    .last_server_frame
                    .map(|f| f == (clock.frame_number - (clock.frames_since_tick - i)))
                    .unwrap_or(false)
                {
                    self.last_server_frame = None;
                    self.client_sim.clone_from(&self.server);
                } else {
                    self.client_sim.update(frame_time, clock.delta_time);
                }
                let t = frame_time.as_secs_f32();
                self.interpolation_buffer.add(splines::Key::new(
                    t,
                    self.client_sim.send_state().clone(),
                    splines::Interpolation::Linear,
                ));
            }
            // sample the simulation at (now - time_per_frame), while offsetting render time into local time
            let t = (time.absolute_time()
                - Duration::new(
                    (-clock.clock_offset_secs) as u64,
                    (-clock.clock_offset_nanos) as u32,
                ))
            .as_secs_f32()
                - clock.time_per_frame.unwrap().as_secs_f32()
                - (self.render_delay / 1000.);
            let pos = self
                .interpolation_buffer
                .sample(t)
                .map(|x| self.client_sim.pos_sample(&x));
            pos
        } else {
            None
        }
    }
    fn update_server(&mut self, time: &Time) -> Sample {
        self.server.update(time.absolute_time(), time.delta_time());
        self.server.pos_sample(self.server.send_state())
    }
}

pub trait DeterministicSimulation: fmt::Debug + Default + Send + Sync + Clone + 'static {
    type SyncType: Serialize
        + for<'de> Deserialize<'de>
        + splines::Interpolate<f32>
        + Send
        + Sync
        + Clone;
    fn send_state(&self) -> &Self::SyncType;
    fn recv_state(&mut self, val: Self::SyncType);
    fn update(&mut self, abs_time: Duration, delta_time: Duration);
    fn pos_sample(&self, val: &Self::SyncType) -> Sample;
    fn initial(settings: &SimSettings) -> Self;
}

pub fn behaviour_data<T: SimulationBehaviour + Default + std::fmt::Display>(
) -> (Arc<dyn SimulationBehaviour>, std::ffi::CString) {
    (
        Arc::new(T::default()),
        std::ffi::CString::new(format!("{}", T::default())).unwrap(),
    )
}
impl<M: Debug + Clone + fmt::Display> fmt::Display for WorldFrame<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{ {:?}
               sample: {} -- {}
             }}",
            self.side, self.render_time, self.sample,
        )
    }
}

#[derive(Debug)]
pub struct SimulationResult<M: Debug + Clone> {
    pub frames: Vec<WorldFrame<M>>,
}

pub fn run_simulation(settings: &SimSettings) -> Result<SimulationResult<Sample>> {
    let (client_tx, server_rx) = memory_channel();
    let (server_tx, client_rx) = memory_channel();
    let server_data = GameDataBuilder::default()
        .with_bundle(MemoryNetworkBundle::new(server_tx, server_rx))?
        .with_system_desc(ServerSimulationSystem, "server_sim", &[]);
    let client_data = GameDataBuilder::default()
        .with_bundle(MemoryNetworkBundle::new(client_tx, client_rx))?
        .with_system_desc(ClientSimulationSystemDesc, "client_sim", &[]);
    let assets_dir = application_root_dir()?.join("./");
    let mut client_monkey = amethyst::network::simulation::NetworkMonkey::new([0; 16]);
    let mut server_monkey = amethyst::network::simulation::NetworkMonkey::new([0; 16]);
    client_monkey.set_min_latency(Some(settings.min_latency / 1000.));
    client_monkey.set_max_latency(Some(settings.max_latency / 1000.));
    client_monkey.set_loss_percentage(Some(settings.loss_percentage));
    server_monkey.set_min_latency(Some(settings.min_latency / 1000.));
    server_monkey.set_max_latency(Some(settings.max_latency / 1000.));
    server_monkey.set_loss_percentage(Some(settings.loss_percentage));
    let sim_result = Arc::new(Mutex::new(SimulationResult { frames: Vec::new() }));
    {
        let mut server_app =
            Application::build(assets_dir.clone(), ServerState::default())?.build(server_data)?;
        let mut client_app =
            Application::build(assets_dir.clone(), ClientState::default())?.build(client_data)?;
        server_app.initialize();
        client_app.initialize();
        server_app.world.insert(settings.clone());
        client_app.world.insert(settings.clone());
        server_app
            .world
            .insert(settings.behaviour.new_state(&settings));
        client_app
            .world
            .insert(settings.behaviour.new_state(&settings));
        server_app.world.insert(sim_result.clone());
        client_app.world.insert(sim_result.clone());
        server_app
            .world
            .get_mut::<NetworkSimulationTime>()
            .unwrap()
            .set_sim_frame_rate(settings.sync_rate as u32);
        client_app
            .world
            .get_mut::<NetworkSimulationTime>()
            .unwrap()
            .set_sim_frame_rate(settings.sync_rate as u32);
        client_app
            .world
            .get_mut::<TransportResource>()
            .unwrap()
            .set_monkey(Some(client_monkey));
        server_app
            .world
            .get_mut::<TransportResource>()
            .unwrap()
            .set_monkey(Some(server_monkey));
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::SmallRng::from_seed([0; 16]);
        let extended_client_duration =
            (settings.render_interpolation_delay + settings.min_latency) / 1000.;
        let mut server_time = settings.duration + extended_client_duration;
        let mut client_time = settings.duration + extended_client_duration;
        while server_time > 0. || client_time > 0. {
            if server_time >= client_time && server_time > 0. {
                let server_delta = 1 as f32 / settings.server_fps as f32;
                server_time -= server_delta;
                server_app.step(Duration::from_secs_f32(server_delta));
            } else if client_time > 0. {
                let render_time_variance = {
                    let deviation = (settings.render_time_variance / 1000.) * 0.5;
                    rng.sample(rand::distributions::Normal::new(0., deviation as f64)) as f32
                };
                let mut client_delta = 1 as f32 / settings.render_fps as f32;
                client_delta += render_time_variance;
                client_time -= client_delta;
                client_app.step(Duration::from_secs_f32(client_delta));
            }
        }
        server_app.shutdown();
        client_app.shutdown();
    }
    Ok(Arc::try_unwrap(sim_result).unwrap().into_inner().unwrap())
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct Sample {
    pub pos: Vector2<f32>,
}

impl fmt::Display for Sample {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "pos: [ x: {}, y: {} ] ", self.pos.x, self.pos.y,)
    }
}

#[derive(Default, Debug)]
pub struct ServerSimulationSystem;

impl<'a, 'b> SystemDesc<'a, 'b, ServerSimulationSystem> for ServerSimulationSystem {
    fn build(self, world: &mut World) -> ServerSimulationSystem {
        world.insert(Sample { pos: math::zero() });
        ServerSimulationSystem
    }
}
impl<'a> System<'a> for ServerSimulationSystem {
    type SystemData = (
        Read<'a, NetworkSimulationTime>,
        Read<'a, Time>,
        Write<'a, TransportResource>,
        WriteExpect<'a, Box<dyn SimulationState>>,
        WriteExpect<'a, Arc<Mutex<SimulationResult<Sample>>>>,
        ReadExpect<'a, SimSettings>,
    );
    fn run(&mut self, (net_time, time, mut transport, mut obj, sim, settings): Self::SystemData) {
        let obj = &mut *obj;
        let sample = obj.update_server(&time);
        for _ in net_time.sim_frames_to_run() {
            let buf = obj.send_sync(&time);
            let server_msg = ServerMessage {
                server_secs: time.absolute_time().as_secs(),
                server_nanos: time.absolute_time().subsec_nanos(),
                server_frame: time.frame_number(),
                msg: buf,
            };
            transport.send(
                std::net::SocketAddr::new("0.0.0.0".parse().unwrap(), 0),
                &bincode::serialize(&server_msg).unwrap(),
            );
        }
        transport.update_monkey(&*time);
        if time.absolute_time().as_secs_f32() <= settings.duration {
            let mut sim = sim.lock().unwrap();
            sim.frames.push(WorldFrame {
                side: SimSide::Server,
                render_time: time.absolute_time().as_secs_f32(),
                net_time: (time.absolute_time() + net_time.elapsed_duration()).as_secs_f32(),
                sample,
            });
        }
    }
}
pub struct ClientSimulationSystem {
    reader: ReaderId<NetworkSimulationEvent>,
}
pub struct ClientSimulationSystemDesc;

impl<'a, 'b> SystemDesc<'a, 'b, ClientSimulationSystem> for ClientSimulationSystemDesc {
    fn build(self, world: &mut World) -> ClientSimulationSystem {
        world.insert(Sample { pos: math::zero() });
        let has_chan = world
            .try_fetch_mut::<EventChannel<NetworkSimulationEvent>>()
            .is_some();
        if !has_chan {
            world.insert(EventChannel::<NetworkSimulationEvent>::default());
        }
        let mut chan = world.fetch_mut::<EventChannel<NetworkSimulationEvent>>();
        let reader = chan.register_reader();
        ClientSimulationSystem { reader }
    }
}
impl<'a> System<'a> for ClientSimulationSystem {
    type SystemData = (
        Read<'a, NetworkSimulationTime>,
        Read<'a, Time>,
        WriteExpect<'a, Box<dyn SimulationState>>,
        Read<'a, EventChannel<NetworkSimulationEvent>>,
        WriteExpect<'a, Arc<Mutex<SimulationResult<Sample>>>>,
    );
    fn run(&mut self, (net_time, time, mut obj, channel, sim): Self::SystemData) {
        let mut sim = sim.lock().unwrap();
        let obj = &mut *obj;
        for event in channel.read(&mut self.reader) {
            match event {
                NetworkSimulationEvent::Message(_, payload) => {
                    let server_msg: ServerMessage = bincode::deserialize(&payload).unwrap();
                    obj.recv_sync(
                        &time,
                        Duration::new(server_msg.server_secs, server_msg.server_nanos),
                        server_msg.server_frame,
                        &server_msg.msg,
                    );
                }
                _ => {}
            }
        }
        if let Some(sample) = obj.update_render(&time) {
            sim.frames.push(WorldFrame {
                side: SimSide::Client,
                render_time: time.absolute_time().as_secs_f32(),
                net_time: (time.absolute_time() + net_time.elapsed_duration()).as_secs_f32(),
                sample,
            });
        }
    }
}

struct ServerState {}
impl Default for ServerState {
    fn default() -> Self {
        Self {}
    }
}

impl SimpleState for ServerState {
    fn update(&mut self, _data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        Trans::None
    }
}

struct ClientState;
impl Default for ClientState {
    fn default() -> Self {
        Self {}
    }
}

impl SimpleState for ClientState {}
