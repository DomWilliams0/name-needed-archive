use std::time::{Duration, Instant};

use simulation::input::InputCommand;
use simulation::{
    Exit, InitializedSimulationBackend, PerfAvg, PersistentSimulationBackend, RenderComponent,
    Renderer, Simulation, TransformComponent, WorldViewer,
};

pub struct DummyRenderer;

pub struct DummyBackendPersistent;
pub struct DummyBackendInit {
    end_time: Instant,
    world_viewer: WorldViewer,
}

impl Renderer for DummyRenderer {
    type Target = ();
    type Error = ();

    fn init(&mut self, _target: Self::Target) {}

    fn sim_start(&mut self) {}

    fn sim_entity(&mut self, _transform: &TransformComponent, _render: &RenderComponent) {}

    fn sim_selected(&mut self, _transform: &TransformComponent) {}

    fn sim_finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn deinit(&mut self) -> Self::Target {}
}

impl InitializedSimulationBackend for DummyBackendInit {
    type Renderer = DummyRenderer;
    type Persistent = DummyBackendPersistent;

    fn consume_events(&mut self) -> Option<Exit> {
        if Instant::now() > self.end_time {
            Some(Exit::Stop)
        } else {
            None
        }
    }

    fn tick(&mut self) {}

    fn render(
        &mut self,
        _: &mut Simulation<Self::Renderer>,
        _: f64,
        _: &PerfAvg,
        _: &mut Vec<InputCommand>,
    ) {
    }

    fn world_viewer(&mut self) -> &mut WorldViewer {
        &mut self.world_viewer
    }

    fn end(self) -> Self::Persistent {
        DummyBackendPersistent
    }
}

impl PersistentSimulationBackend for DummyBackendPersistent {
    type Error = ();
    type Initialized = DummyBackendInit;

    fn new() -> Result<Self, Self::Error> {
        Ok(Self)
    }

    fn start(self, world_viewer: WorldViewer) -> Self::Initialized {
        DummyBackendInit {
            end_time: Instant::now() + Duration::from_secs(30),
            world_viewer,
        }
    }
}
