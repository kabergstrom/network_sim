use crate::sim::{Sample, SimSettings, SimSide, SimulationResult, WorldFrame};

use amethyst::{
    core::math::{Point3, Vector2},
    ecs::{ReadExpect, Write, WriteExpect},
    renderer::{debug_drawing::DebugLines, palette::Srgba},
    window::ScreenDimensions,
};
use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};
fn sim_bounding_box_render<M: Debug + Clone>(
    sim: &SimulationResult<M>,
    map_fn: fn(&WorldFrame<M>) -> Vector2<f32>,
) -> (Vector2<f32>, Vector2<f32>) {
    let min_pos_x = sim
        .frames
        .iter()
        .map(map_fn)
        .map(|pos| pos.x)
        .fold(std::f32::INFINITY, f32::min);
    let mut max_pos_x = sim
        .frames
        .iter()
        .map(map_fn)
        .map(|pos| pos.x)
        .fold(std::f32::NEG_INFINITY, f32::max);
    let mut max_pos_y = sim
        .frames
        .iter()
        .map(map_fn)
        .map(|pos| pos.y)
        .fold(std::f32::NEG_INFINITY, f32::max);
    let min_pos_y = sim
        .frames
        .iter()
        .map(map_fn)
        .map(|pos| pos.y)
        .fold(std::f32::INFINITY, f32::min);
    if max_pos_x == min_pos_x {
        max_pos_x = min_pos_x + 1.;
    }
    if max_pos_y == min_pos_y {
        max_pos_y = min_pos_y + 1.;
    }
    (
        Vector2::new(min_pos_x, min_pos_y),
        Vector2::new(max_pos_x, max_pos_y),
    )
}
pub struct SimRenderSystem;
impl<'s> amethyst::ecs::System<'s> for SimRenderSystem {
    type SystemData = (
        ReadExpect<'s, ScreenDimensions>,
        Write<'s, DebugLines>,
        WriteExpect<'s, Arc<Mutex<SimulationResult<Sample>>>>,
        WriteExpect<'s, SimSettings>,
    );
    fn run(&mut self, (screen_dimensions, mut lines, sim, settings): Self::SystemData) {
        let sim = sim.lock().unwrap();
        let screen_w = screen_dimensions.width();
        let screen_h = screen_dimensions.height();

        let (min_pos, max_pos) = sim_bounding_box_render(&sim, |x| x.sample.pos);
        // lines.draw_line(
        //     Point3::new(screen_w * 0.5, screen_h * 0.5, 0.),
        //     Point3::new(screen_w, screen_h, 0.),
        //     Srgba::new(0.3, 0.3, 1.0, 1.0),
        // );
        // lines.draw_line(
        //     Point3::new(screen_w * 0.75, screen_h * 0.1, 0.),
        //     Point3::new(screen_w, screen_h * 0.1, 0.),
        //     Srgba::new(1.0, 0.3, 0.3, 1.0),
        // );
        // lines.draw_line(
        //     Point3::new(screen_w * 0.5, screen_h * 0.2, 0.),
        //     Point3::new(screen_w * 0.75, screen_h * 0.2, 0.),
        //     Srgba::new(0.3, 1.0, 1.0, 1.0),
        // );
        // lines.draw_line(
        //     Point3::new(screen_w * 0.25, screen_h * 0.3, 0.),
        //     Point3::new(screen_w * 0.5, screen_h * 0.3, 0.),
        //     Srgba::new(0.3, 1., 0., 1.0),
        // );
        // lines.draw_line(
        //     Point3::new(screen_w * 0., screen_h * 0.4, 0.),
        //     Point3::new(screen_w * 0.25, screen_h * 0.4, 0.),
        //     Srgba::new(0.3, 0.3, 1.0, 1.0),
        // );
        let render_size = Vector2::new(screen_w * 0.45, screen_h * 0.85);
        let mut server_pos_color = None;
        let mut client_pos_color = None;
        for frame in sim.frames.iter() {
            let pos = (frame.sample.pos - min_pos)
                .component_div(&(max_pos - min_pos))
                .component_mul(&render_size);
            let (pos, color) = match frame.side {
                SimSide::Server => (
                    Point3::new(pos.x + screen_w * 0.02, pos.y + screen_h * 0.02, 0.0),
                    Srgba::new(0.3, 0.3, 1.0, 1.0),
                ),
                SimSide::Client => (
                    Point3::new(pos.x + screen_w * 0.5, pos.y + screen_h * 0.02, 0.0),
                    Srgba::new(0.7, 0.3, 0.3, 1.0),
                ),
            };
            lines.draw_circle(pos, 1.0, 2, color);
            if frame.render_time <= settings.curr_time {
                match frame.side {
                    SimSide::Server => server_pos_color = Some((pos, color)),
                    SimSide::Client => client_pos_color = Some((pos, color)),
                }
            }
        }
        if let Some((pos, color)) = server_pos_color {
            lines.draw_circle(pos, 5.0, 10, color);
        }
        if let Some((pos, color)) = client_pos_color {
            lines.draw_circle(pos, 5.0, 10, color);
        }
    }
}
