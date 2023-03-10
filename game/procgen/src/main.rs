use common::*;
use std::io::Write;
use std::time::SystemTime;

#[allow(dead_code)]
fn log_time(out: &mut dyn Write) -> std::io::Result<()> {
    lazy_static! {
        static ref START_TIME: SystemTime = SystemTime::now();
    }

    let now = SystemTime::now();
    write!(
        out,
        "{:8}",
        now.duration_since(*START_TIME)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    )
}

#[cfg(feature = "bin")]
fn main() {
    use procgen::*;

    let logger = logging::LoggerBuilder::with_env("NN_LOG")
        .and_then(|builder| builder.init(log_time))
        .expect("logging failed");
    info!("initialized logging"; "level" => ?logger.level());

    // parse config and args first
    let params = PlanetParams::load_file_with_args("procgen.txt");

    let exit = match params {
        Err(err) => {
            error!("failed to parse params: {}", err);
            1
        }
        Ok(params) if params.log_params_and_exit => {
            // nop
            info!("config: {:#?}", params);
            0
        }
        Ok(params) => {
            info!("config: {:#?}", params);

            let dew_it = || {
                use tokio::runtime as rt;
                let runtime = if params.render.threads == 1 {
                    rt::Builder::new_current_thread()
                } else {
                    rt::Builder::new_multi_thread()
                }
                .worker_threads(params.render.threads)
                .enable_time()
                .build()
                .expect("failed to create runtime");

                runtime.block_on(async {
                    let mut planet = Planet::new(params.clone()).expect("failed");
                    planet.initial_generation().await.expect("failed");

                    let mut render = Render::with_planet(planet.clone()).await;
                    render.draw_continents().await;
                    render.save("procgen.png").expect("failed to write image");

                    for y in 64..65 {
                        for x in 4..5 {
                            let region = RegionLocation::new(x, y);

                            let mut render = Render::with_planet(planet.clone()).await;
                            if let Err(err) = render.draw_region(region).await {
                                error!("bad slab: {}", err);
                                break;
                            }
                            render
                                .save(format!("procgen-region-{}-{}.png", x, y))
                                .expect("failed to write image");
                        }
                    }
                })
            };
            match panik::Builder::new()
                .slogger(logger.logger())
                .run_and_handle_panics(dew_it)
            {
                Some(_) => 0,
                None => 1,
            }
        }
    };

    // let logging end gracefully
    info!("all done");
    drop(logger);
    std::thread::sleep(std::time::Duration::from_secs(1));

    std::process::exit(exit);
}

#[cfg(not(feature = "bin"))]
fn main() {
    unreachable!("missing feature \"bin\"")
}
