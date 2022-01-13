use simulation::input::{UiCommand, UiCommands, UiRequest, UiResponse};
use simulation::{ComponentRef, ComponentWorld, Entity, KindComponent, PerfAvg, SimulationRef};

use crate::render::sdl::ui::memory::PerFrameStrings;
use imgui::{
    ImStr, TabBar, TabBarFlags, TabBarToken, TabItem, TabItemToken, TreeNode, TreeNodeToken, Ui,
};
use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::Deref;

/// Context for a single frame. Provides communication to the game
pub struct UiContext<'ctx> {
    ui: &'ctx imgui::Ui<'ctx>,
    strings: &'ctx PerFrameStrings,
    perf: PerfAvg,
    simulation: SimulationRef<'ctx>,
    commands: RefCell<&'ctx mut UiCommands>,
}

/// # Safety
/// No UI reference is actually passed, so implementations must ensure that that param is unused
pub unsafe trait UiGuardable: Sized {
    fn end(self, null_ui: &imgui::Ui);
}

#[must_use]
pub struct UiGuard<T: UiGuardable>(Option<T>);

pub enum DefaultOpen {
    Closed,
    Open,
}

pub enum EntityDesc<'a> {
    Kind(ComponentRef<'a, KindComponent>),
    Overridden(&'a dyn Display),
    Fallback(Entity),
}

impl<'a> UiContext<'a> {
    pub fn new(
        ui: &'a imgui::Ui<'a>,
        strings: &'a PerFrameStrings,
        simulation: SimulationRef<'a>,
        commands: &'a mut UiCommands,
        perf: PerfAvg,
    ) -> Self {
        Self {
            ui,
            strings,
            perf,
            simulation,
            commands: RefCell::new(commands),
        }
    }

    pub fn ui(&self) -> &'a imgui::Ui<'a> {
        self.ui
    }

    pub fn strings(&self) -> &PerFrameStrings {
        self.strings
    }

    pub fn perf(&self) -> &PerfAvg {
        &self.perf
    }

    pub fn simulation(&self) -> &SimulationRef {
        &self.simulation
    }

    pub fn issue_request(&self, req: UiRequest) -> UiResponse {
        let command = UiCommand::new(req);
        let response = command.response();
        self.commands.borrow_mut().push(command);
        response
    }

    /// Helper to reduce insane nesting in [build] closures. Must check [UiGuard::is_open]!!
    /// Returned guard should be dropped before creating a new one (not ideal)
    pub fn new_tab(&self, title: &ImStr) -> UiGuard<TabItemToken> {
        UiGuard(TabItem::new(title).begin(self.ui))
    }

    /// Helper to reduce insane nesting in [build] closures. Must check [UiGuard::is_open]!!
    /// Returned guard should be dropped before creating a new one (not ideal)
    pub fn new_tab_bar(&self, id: &ImStr) -> UiGuard<TabBarToken> {
        UiGuard(
            TabBar::new(id)
                .flags(TabBarFlags::FITTING_POLICY_SCROLL)
                .begin(self.ui),
        )
    }

    /// Helper to reduce insane nesting in [build] closures. Must check [UiGuard::is_open]!!
    /// Returned guard should be dropped before creating a new one (not ideal)
    pub fn new_tree_node(&self, title: &ImStr, open: DefaultOpen) -> UiGuard<TreeNodeToken> {
        UiGuard(
            TreeNode::new(title)
                .default_open(matches!(open, DefaultOpen::Open))
                .push(self.ui),
        )
    }

    pub fn description(&self, e: Entity) -> EntityDesc {
        match self.simulation.ecs.component::<KindComponent>(e) {
            Ok(comp) => EntityDesc::Kind(comp),
            Err(_) => EntityDesc::Fallback(e),
        }
    }
}

impl Display for EntityDesc<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let display = match self {
            EntityDesc::Kind(kind) => &**kind as &dyn Display,
            EntityDesc::Overridden(d) => d,
            EntityDesc::Fallback(e) => e as &dyn Display,
        };

        Display::fmt(display, f)
    }
}

impl<'a> EntityDesc<'a> {
    pub fn with_fallback(self, fallback: &'a dyn Display) -> Self {
        match self {
            EntityDesc::Fallback(_) => EntityDesc::Overridden(fallback),
            other => other,
        }
    }
}

impl<T: UiGuardable> UiGuard<T> {
    pub fn is_open(&self) -> bool {
        self.0.is_some()
    }
}

impl<T: UiGuardable> Drop for UiGuard<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take() {
            let null_ui = MaybeUninit::<Ui>::uninit();
            // safety: implementation guarantees this is unused
            let null_ui = ManuallyDrop::new(unsafe { null_ui.assume_init() });
            inner.end(&null_ui);
        }
    }
}

impl<'a> Deref for UiContext<'a> {
    type Target = imgui::Ui<'a>;

    fn deref(&self) -> &Self::Target {
        self.ui
    }
}

unsafe impl UiGuardable for TabItemToken {
    fn end(self, null_ui: &Ui) {
        TabItemToken::end(self, null_ui)
    }
}

unsafe impl UiGuardable for TreeNodeToken {
    fn end(self, null_ui: &Ui) {
        self.pop(null_ui)
    }
}

unsafe impl UiGuardable for TabBarToken {
    fn end(self, null_ui: &Ui) {
        self.end(null_ui)
    }
}
