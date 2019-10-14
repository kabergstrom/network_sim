#![feature(const_fn)]
use amethyst::{
    core::transform::{Transform, TransformBundle},
    prelude::*,
    renderer::{
        camera::Camera,
        debug_drawing::{DebugLines, DebugLinesParams},
        plugins::{RenderDebugLines, RenderToWindow},
        types::DefaultBackend,
        RenderingBundle,
    },
    utils::application_root_dir,
    window::ScreenDimensions,
    Result,
};
use std::net::TcpListener;

mod control;
mod render;
mod sim;
mod sim_behaviours;

use control::GuiSystemDesc;
use render::SimRenderSystem;

fn main() -> Result<()> {
    use amethyst::LoggerConfig;
    amethyst::start_logger(LoggerConfig {
        level_filter: log::LevelFilter::Warn,
        ..Default::default()
    });

    let listener = TcpListener::bind("0.0.0.0:3457")?;
    listener.set_nonblocking(true).unwrap();

    let assets_dir = application_root_dir()?.join("./");

    let app_root = application_root_dir()?;

    let display_config_path = app_root.join("config/display.ron");

    let render_data = GameDataBuilder::default()
        .with_bundle(TransformBundle::new())?
        .with_barrier()
        .with(SimRenderSystem, "sim_render", &[])
        .with_system_desc(GuiSystemDesc, "gui_system", &[])
        .with_bundle(amethyst::input::InputBundle::<
            amethyst::input::StringBindings,
        >::default())?
        .with_bundle(
            RenderingBundle::<DefaultBackend>::new()
                .with_plugin(
                    RenderToWindow::from_config_path(display_config_path)
                        .with_clear([0.0, 0.0, 0.0, 1.0]),
                )
                .with_plugin(RenderDebugLines::default())
                .with_plugin(
                    amethyst_imgui::RenderImgui::<amethyst::input::StringBindings>::default(),
                ),
        )?;
    let mut render_app = Application::build(assets_dir.clone(), RenderState)?.build(render_data)?;
    render_app.run();
    Ok(())
}

struct RenderState;

impl SimpleState for RenderState {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        // Setup debug lines as a resource
        data.world.insert(DebugLines::new());
        // Configure width of lines. Optional step
        data.world.insert(DebugLinesParams { line_width: 1.0 });

        // Setup debug lines as a component and add lines to render axis&grid
        // let mut debug_lines_component = DebugLinesComponent::new();

        let (screen_w, screen_h) = {
            let screen_dimensions = data.world.read_resource::<ScreenDimensions>();
            (
                screen_dimensions.width() * screen_dimensions.hidpi_factor() as f32,
                screen_dimensions.height() * screen_dimensions.hidpi_factor() as f32,
            )
        };

        // for y in (0..(screen_h as u16)).step_by(50).map(f32::from) {
        //     debug_lines_component.add_line(
        //         [0.0, y, 1.0].into(),
        //         [screen_w, (y + 2.0), 1.0].into(),
        //         Srgba::new(0.3, 0.3, 0.3, 1.0),
        //     );
        // }

        // for x in (0..(screen_w as u16)).step_by(50).map(f32::from) {
        //     debug_lines_component.add_line(
        //         [x, 0.0, 1.0].into(),
        //         [x, screen_h, 1.0].into(),
        //         Srgba::new(0.3, 0.3, 0.3, 1.0),
        //     );
        // }

        // debug_lines_component.add_line(
        //     [20.0, 20.0, 1.0].into(),
        //     [780.0, 580.0, 1.0].into(),
        //     Srgba::new(1.0, 0.0, 0.2, 1.0), // Red
        // );

        // data.world
        //     .create_entity()
        //     .with(debug_lines_component)
        //     .build();

        // Setup camera
        let mut local_transform = Transform::default();
        local_transform.set_translation_xyz(screen_w / 2., screen_h / 2., 10.0);
        println!("creating camera with {} {} ", screen_w, screen_h);
        data.world
            .create_entity()
            .with(Camera::standard_2d(screen_w, screen_h))
            .with(local_transform)
            .build();
    }
    fn update(&mut self, data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        let (screen_w, screen_h) = {
            let screen_dimensions = data.world.read_resource::<ScreenDimensions>();
            (
                screen_dimensions.width() * screen_dimensions.hidpi_factor() as f32,
                screen_dimensions.height() * screen_dimensions.hidpi_factor() as f32,
            )
        };
        use amethyst::ecs::Join;
        for camera in (&mut data.world.write_component::<Camera>()).join() {
            *camera = Camera::standard_2d(screen_w, screen_h);
        }
        Trans::None
    }
}
