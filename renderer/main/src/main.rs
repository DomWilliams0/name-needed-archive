use clap::{App, Arg};
use common::*;
use engine::Engine;
use presets::{DevGamePreset, EmptyGamePreset, GamePreset};
use simulation::{ExitType, SimulationBackend};

use std::path::PathBuf;
use std::process::Command;
use struclog::sink::ipc::IpcSink;

mod presets;

fn main() {
    let args = App::new(env!("CARGO_PKG_NAME"))
        .arg(
            Arg::with_name("preset")
                .short("p")
                .long("preset")
                .help("Game preset")
                .takes_value(true)
                .possible_values(&["dev", "empty"]),
        )
        .arg(
            Arg::with_name("dir")
                .short("d")
                .long("dir")
                .help("Directory to look for game files in")
                .default_value("."),
        )
        .get_matches();

    #[cfg(feature = "sdl-glium")]
    type Backend = engine::SdlGliumBackend;

    #[cfg(feature = "lite")]
    type Backend = engine::DummyBackend;

    type Renderer = <Backend as SimulationBackend>::Renderer;

    let preset: Box<dyn GamePreset<Renderer>> = match args.value_of("preset") {
        None | Some("dev") => Box::new(DevGamePreset::<Renderer>::default()),
        Some("empty") => Box::new(EmptyGamePreset::default()),
        _ => unreachable!(),
    };

    let root = args.value_of("dir").unwrap();

    // init logger
    env_logger::Builder::from_env(env_logger::Env::default().filter_or("NN_LOG", "info"))
        .target(env_logger::Target::Stdout)
        .init();

    info!("using game preset '{}'", preset.name());

    // load config
    if let Some(config_file_name) = preset.config() {
        let config_path = {
            let mut path = PathBuf::new();
            path.push(root);
            path.push(config_file_name);
            path
        };

        info!("loading config from '{:?}'", config_path);
        if let Err(e) = config::init(config_path) {
            error!("failed to load config initially: {}", e);
            std::process::exit(1);
        }
    }

    // enable structured logging
    struclog::init(Some(Box::new(IpcSink::default())));

    // and away we go
    let sim = {
        let _span = enter_span(Span::Setup);
        preset.load()
    };
    let engine = Engine::<Renderer, Backend>::new(sim);
    if let ExitType::Restart = engine.run() {
        info!("restarting renderer");
        // TODO preserve camera position and other runtime settings?
        restart();
    }

    info!("exiting cleanly");
}

#[cfg(unix)]
fn restart() -> ! {
    use std::os::unix::process::CommandExt;

    // get current exe without the (deleted) prefix
    let cmd = std::env::current_exe().expect("failed to get current exe");
    let cmd_s = cmd.to_str().expect("bad current exe");
    let exe = cmd_s.split(" (deleted)").next().unwrap();
    let err = Command::new(exe)
        .args(std::env::args_os().skip(1))
        .envs(std::env::vars_os())
        .exec(); // won't return on success
    panic!("failed to restart: {}", err);
}