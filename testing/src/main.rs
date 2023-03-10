/// Test runner
#[allow(dead_code)] // they are used actually
#[derive(argh::FromArgs)]
struct Args {
    /// if graphical renderer should be used
    #[argh(switch)]
    graphical: bool,

    /// timeout in seconds per test
    #[argh(option, default = "30")]
    timeout: u32,

    /// only run tests containing this string in their name
    #[argh(option)]
    filter: Option<String>,

    /// just collect tests, don't run them
    #[argh(switch)]
    dry_run: bool,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 5)]
async fn main() {
    #[cfg(not(feature = "testing"))]
    {
        panic!("\"testing\" feature is needed")
    }

    #[cfg(feature = "testing")]
    if let Err(err) = actual_testing::do_main().await {
        panic!("{}", err)
    }
}

#[cfg(feature = "testing")]
mod actual_testing {
    use super::Args;
    use std::error::Error;
    use tokio::process::Child;

    pub async fn do_main() -> Result<(), Box<dyn Error>> {
        use futures::TryFutureExt;
        use std::time::Duration;

        testing::register_tests();

        let args = argh::from_env::<Args>();
        let renderer = if args.graphical {
            Renderer::Graphical
        } else {
            Renderer::Lite
        };

        let mut tests = inventory::iter::<testing::TestDeclaration>
            .into_iter()
            .collect::<Vec<_>>();

        let summary = {
            let mut s = if let Some(filter) = args.filter {
                let total_count = tests.len();
                let filter = filter.to_lowercase();
                tests.retain(|test| test.name.to_lowercase().contains(&filter));
                format!(
                    "{}/{} tests matching filter '{}':\n",
                    tests.len(),
                    total_count,
                    filter
                )
            } else {
                format!("{} tests:\n", tests.len())
            };

            for test in &tests {
                s.push_str(" - ");
                s.push_str(test.name);
                s.push('\n');
            }

            s.pop(); // last nl

            s
        };
        eprintln!("running {}", summary);

        if args.dry_run {
            return Ok(());
        }

        if tests.is_empty() {
            return Err("no tests".into());
        }

        CargoCommand::Build
            .run(renderer)
            .and_then(wait_on_process)
            .await?;

        for test in tests {
            eprintln!("running test {:?}", test.name);

            let mut test_process = CargoCommand::Run { test: test.name }.run(renderer).await?;
            let result_fut = wait_on_process_ref(&mut test_process);

            let err =
                match tokio::time::timeout(Duration::from_secs(args.timeout as u64), result_fut)
                    .await
                {
                    Ok(Ok(_)) => {
                        eprintln!("test {} passed", test.name);
                        continue;
                    }
                    Ok(Err(err)) => format!("{}", err),
                    Err(_) => "timed out".to_owned(),
                };

            let msg = format!("test {} failed: {}", test.name, err);
            eprintln!("{}", msg);

            // abort test process
            let _ = test_process.kill().await;
            std::process::exit(1);
        }

        eprintln!("successfully ran {}", summary);
        Ok(())
    }

    enum CargoCommand<'a> {
        Build,
        Run { test: &'a str },
    }

    #[derive(Copy, Clone)]
    enum Renderer {
        Lite,
        Graphical,
    }

    impl CargoCommand<'_> {
        async fn run(self, renderer: Renderer) -> Result<Child, Box<dyn Error>> {
            use std::process::Stdio;
            use tokio::process::Command;

            let subcmd = match &self {
                CargoCommand::Build => "build",
                CargoCommand::Run { .. } => "run",
            };

            let mut builder = Command::new(env!("CARGO"));
            if let CargoCommand::Run { test } = self {
                builder.env(testing::TEST_NAME_VAR, test);
            }
            builder
                .args(&[
                    subcmd,
                    "--bin",
                    "main",
                    "--no-default-features",
                    "--features",
                    "tests",
                ])
                .stdin(Stdio::piped());

            let feature = match renderer {
                Renderer::Lite => "lite",
                Renderer::Graphical => "use-sdl",
            };
            builder.args(&["--features", feature]);

            if let CargoCommand::Run { .. } = self {
                builder.args(&["--", "--config", "tests"]);
            }

            builder.spawn().map_err(Into::into)
        }
    }

    async fn wait_on_process(mut process: Child) -> Result<(), Box<dyn Error>> {
        wait_on_process_ref(&mut process).await
    }

    async fn wait_on_process_ref(process: &mut Child) -> Result<(), Box<dyn Error>> {
        let result = process.wait().await?;
        if result.success() {
            Ok(())
        } else {
            // TODO unix special case to get exit code on signal
            Err(format!("cargo exited with code {:?}", result.code()).into())
        }
    }
}
