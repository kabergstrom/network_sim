use crate::sim::{run_simulation, Sample, SimSettings, SimulationResult};

use amethyst::{
    core::Time,
    ecs::{ReadExpect, WriteExpect},
    prelude::*,
    window::ScreenDimensions,
};
use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};
fn sim_min_max_time<M: Debug + Clone>(sim: &SimulationResult<M>) -> (f32, f32) {
    let mut min_time = sim
        .frames
        .iter()
        .map(|x| x.render_time)
        .fold(std::f32::INFINITY, f32::min);
    if min_time == std::f32::INFINITY {
        min_time = 0.;
    }

    let mut max_time = sim
        .frames
        .iter()
        .map(|x| x.render_time)
        .fold(std::f32::NEG_INFINITY, f32::max);
    if max_time == std::f32::NEG_INFINITY {
        max_time = min_time;
    }
    (min_time, max_time)
}

pub struct GuiSystemDesc;

impl<'a, 'b> SystemDesc<'a, 'b, GuiSystem> for GuiSystemDesc {
    fn build(self, world: &mut World) -> GuiSystem {
        let settings = SimSettings::default();
        let sim = run_simulation(&settings).unwrap();
        world.insert(Arc::new(Mutex::new(sim)));
        world.insert(settings);
        GuiSystem
    }
}
pub struct GuiSystem;
impl<'s> amethyst::ecs::System<'s> for GuiSystem {
    type SystemData = (
        ReadExpect<'s, ScreenDimensions>,
        ReadExpect<'s, Time>,
        WriteExpect<'s, Arc<Mutex<SimulationResult<Sample>>>>,
        WriteExpect<'s, SimSettings>,
    );
    fn run(&mut self, (_screen_dimensions, time, sim, mut settings): Self::SystemData) {
        let mut sim = sim.lock().unwrap();
        let (min_time, max_time) = sim_min_max_time(&sim);
        if settings.playing {
            settings.curr_time += time.delta_seconds() * settings.sim_time_scale;
            settings.curr_time = settings.curr_time % max_time;
        }
        amethyst_imgui::with(|ui| {
            use amethyst_imgui::imgui::*;
            Window::new(im_str!("control"))
                .size([550., 400.], Condition::Once)
                .build(ui, || {
                    ui.push_item_width(300.0);
                    Slider::new(im_str!("sim time"), min_time..=max_time)
                        .build(ui, &mut settings.curr_time);
                    Slider::new(im_str!("sim time scale"), 0.1..=1.)
                        .build(ui, &mut settings.sim_time_scale);
                    let mut changed = Slider::new(im_str!("server fps"), 1..=240)
                        .build(ui, &mut settings.server_fps);
                    changed |= Slider::new(im_str!("client fps"), 1..=240)
                        .build(ui, &mut settings.render_fps);
                    changed |= Slider::new(im_str!("sync rate"), 1..=240)
                        .build(ui, &mut settings.sync_rate);
                    changed |= Slider::new(im_str!("render interpolation delay ms"), 0.0..=500.0)
                        .build(ui, &mut settings.render_interpolation_delay);
                    let max_variance = (1000.0 / settings.render_fps as f32) * 0.5;
                    changed |= Slider::new(im_str!("render time variance ms"), 0.0..=max_variance)
                        .build(ui, &mut settings.render_time_variance);
                    if settings.render_time_variance > max_variance {
                        settings.render_time_variance = max_variance;
                    }
                    changed |= Slider::new(im_str!("min latency ms"), 0.0..=500.0)
                        .build(ui, &mut settings.min_latency);
                    if settings.min_latency > settings.max_latency {
                        settings.max_latency = settings.min_latency;
                    }
                    changed |= Slider::new(im_str!("max latency ms"), 0.0..=500.0)
                        .build(ui, &mut settings.max_latency);
                    if settings.min_latency > settings.max_latency {
                        settings.min_latency = settings.max_latency;
                    }
                    changed |= Slider::new(im_str!("loss percentage"), 0.0..=1.0)
                        .build(ui, &mut settings.loss_percentage);
                    changed |= Slider::new(im_str!("sim duration"), 0.1..=5.0)
                        .build(ui, &mut settings.duration);
                    let toggle_playing = if settings.playing {
                        ui.small_button(im_str!("Pause"))
                    } else {
                        ui.small_button(im_str!("Play"))
                    };
                    changed |= if ui.small_button(im_str!("Reset")) {
                        *settings = SimSettings::default();
                        true
                    } else {
                        false
                    };
                    if toggle_playing {
                        settings.playing = !settings.playing;
                    }
                    let current_id = settings.behaviour.type_id();
                    let mut selected_idx = crate::sim_behaviours::SIM_BEHAVIOURS
                        .iter()
                        .position(|x| x.0.type_id() == current_id)
                        .unwrap_or(0);
                    if ComboBox::new(im_str!("Mode")).build_simple(
                        ui,
                        &mut selected_idx,
                        &crate::sim_behaviours::SIM_BEHAVIOURS,
                        &|x| unsafe {
                            std::borrow::Cow::Borrowed(ImStr::from_cstr_unchecked(x.1.as_c_str()))
                        },
                    ) {
                        changed = true;
                        settings.behaviour = crate::sim_behaviours::SIM_BEHAVIOURS[selected_idx]
                            .0
                            .clone();
                    }
                    if changed {
                        let new_sim = run_simulation(&settings).unwrap();
                        *sim = new_sim;
                    }
                });
        });
    }
}
