use common::*;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;
use structopt::StructOpt;
use strum_macros::EnumString;

#[derive(Debug, Clone, StructOpt, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct PlanetParams {
    /// Random if not specified
    #[structopt(long)]
    seed: Option<u64>,

    /// Height and width of surface in some unit
    #[structopt(long, default_value = "128")]
    pub planet_size: u32,

    #[structopt(long, default_value = "5")]
    pub max_continents: usize,

    #[structopt(long, default_value = "10.0")]
    pub continent_start_radius: f32,

    #[structopt(long, default_value = "0.1")]
    pub continent_dec_min: f32,

    #[structopt(long, default_value = "0.3")]
    pub continent_dec_max: f32,

    #[structopt(long, default_value = "20")]
    pub continent_min_distance: i32,

    #[cfg(feature = "bin")]
    #[structopt(flatten)]
    pub render: RenderParams,

    #[structopt(long)]
    pub log_params_and_exit: bool,
}

#[derive(Debug, Clone, EnumString, Deserialize)]
#[strum(serialize_all = "kebab-case")]
pub enum RenderProgressParams {
    None,

    #[strum(serialize = "temp")]
    Temperature,
}

#[derive(Debug, Clone, StructOpt, Deserialize)]
#[structopt(rename_all = "kebab-case")]
pub struct RenderParams {
    #[structopt(long)]
    pub draw_debug_colors: bool,

    #[structopt(long)]
    pub draw_continent_blobs: bool,

    #[structopt(long)]
    pub draw_continent_blobs_outline: bool,

    #[structopt(long)]
    pub draw_density: bool,

    #[structopt(long)]
    pub draw_height: bool,

    #[structopt(long, default_value = "None")]
    pub draw_progress: RenderProgressParams,

    #[structopt(long)]
    pub create_climate_gif: bool,
}

impl PlanetParams {
    pub fn load_with_args(file_path: impl AsRef<Path>) -> BoxedResult<Self> {
        Self::load(file_path.as_ref(), std::env::args())
    }

    pub fn load_with_only_file(file_path: impl AsRef<Path>) -> BoxedResult<Self> {
        let fake_args = once(env!("CARGO_PKG_NAME").to_owned());
        Self::load(file_path.as_ref(), fake_args)
    }

    // TODO return a result instead of panicking
    /// Must be at least len 1, where first elem is binary name
    fn load(file_path: &Path, mut args: impl Iterator<Item = String>) -> BoxedResult<Self> {
        let mut params = {
            let mut config_params = Vec::new();

            // binary name
            config_params.push(args.next().expect("no 0th arg"));

            match File::open(file_path) {
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    // no file, no problem
                    warn!(
                        "couldn't find config file at '{}', continuing with defaults",
                        file_path.display()
                    );
                }
                Err(e) => panic!("failed to read config file: {}", e),
                Ok(file) => {
                    let lines = BufReader::new(file);
                    for line in lines.lines().filter_map(|line| line.ok()).filter(|line| {
                        let trimmed = line.trim();
                        !trimmed.is_empty() && !trimmed.starts_with('#')
                    }) {
                        config_params.extend(line.split(' ').map(str::to_owned));
                    }
                }
            };

            // binary name || args from file || args from cmdline
            Self::from_iter_safe(config_params.into_iter().chain(args))?
        };

        // generate random seed
        if params.seed.is_none() {
            params.seed = Some(thread_rng().gen())
        }

        Ok(params)
    }

    pub fn seed(&self) -> u64 {
        self.seed.expect("seed should have been initialized")
    }
}
