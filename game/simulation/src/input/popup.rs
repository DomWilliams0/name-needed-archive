use unit::world::WorldPoint;

use crate::input::popup::content::Button;
use crate::{EcsWorld, Entity};

/// Single right click context menu
#[derive(Default)]
pub struct UiPopup {
    popup: Option<PopupContent>,
}

pub struct PreparedUiPopup<'a>(&'a mut UiPopup);

#[derive(Copy, Clone)]
pub enum PopupContentType {
    TileSelection,
    TargetEntity(Entity),
    TargetPoint(WorldPoint),
}

pub struct PopupContent {
    ty: PopupContentType,
    content: Option<RenderedPopupContent>,
}

// TODO bump alloc
pub struct RenderedPopupContent {
    title: String,
    buttons: Vec<Button>,
}

impl UiPopup {
    /// Opened at mouse position
    pub fn open(&mut self, content: PopupContentType) {
        self.popup = Some(PopupContent {
            ty: content,
            content: None,
        });
    }

    fn on_close(&mut self) {
        self.popup = None;
    }

    /// Returns true if closed
    pub fn close(&mut self) -> bool {
        if self.popup.is_some() {
            self.popup = None;
            true
        } else {
            false
        }
    }

    /// Called once per frame by render system
    pub fn prepare(&mut self) -> PreparedUiPopup {
        PreparedUiPopup(self)
    }
}

impl RenderedPopupContent {
    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn buttons(&mut self) -> impl Iterator<Item = &mut Button> + ExactSizeIterator {
        self.buttons.iter_mut()
    }
}

impl PopupContentType {
    fn prepare(&self, world: &EcsWorld) -> RenderedPopupContent {
        content::prepare_popup(*self, world)
    }
}

impl PopupContent {
    pub fn as_renderable(&mut self, world: &EcsWorld) -> (&mut RenderedPopupContent, bool) {
        let open = if self.content.is_none() {
            // prepare for rendering
            self.content = Some(self.ty.prepare(world));
            true
        } else {
            false
        };

        debug_assert!(self.content.is_some());
        // safety: unconditionally set above
        let content = unsafe { self.content.as_mut().unwrap_unchecked() };

        (content, open)
    }
}

impl PreparedUiPopup<'_> {
    pub fn iter_all(&mut self) -> impl Iterator<Item = &mut PopupContent> + '_ {
        self.0.popup.as_mut().into_iter()
    }

    pub fn on_close(&mut self) {
        self.0.on_close()
    }
}

mod content {
    use std::fmt;
    use std::fmt::Display;
    use std::rc::Rc;

    use common::SmallVec;
    use unit::world::{WorldPoint, WorldPositionRange};

    use crate::ai::AiComponent;
    use crate::ecs::*;
    use crate::input::popup::{PopupContentType, RenderedPopupContent};
    use crate::input::{SelectedEntities, SelectedTiles, UiRequest, UiResponse};
    use crate::item::HaulableItemComponent;
    use crate::job::{SocietyCommand, SocietyJobHandle};
    use crate::{
        AiAction, BuildTemplate, ContainedInComponent, FollowPathComponent, HaulPurpose,
        HaulSource, HaulTarget, PlayerSociety, SocietyComponent, SocietyHandle, UiElementComponent,
        WorldRef,
    };

    pub enum ButtonType {
        GoTo(WorldPoint),
        Follow(Entity),
        // TODO prioritise job
        CancelJobs(SmallVec<[SocietyJobHandle; 1]>),
        CancelDivineCommand,
        /// Society command or divine command to all subjects
        Command(Option<SocietyHandle>, ButtonCommand),
        /// Haul multiple entities to target pos
        HaulToTarget(SocietyHandle, SmallVec<[Entity; 1]>, WorldPoint),
        Build {
            society: SocietyHandle,
            range: WorldPositionRange,
            template: Rc<BuildTemplate>,
            display: Rc<dyn Display>,
            outline_only: bool,
        },
    }

    /// Individual divine command or society
    #[derive(Clone)]
    pub enum ButtonCommand {
        /// Haul single entity to tile selection
        HaulToPosition(Entity, WorldPoint),
        /// Only works for single block for divine
        BreakBlocks(WorldPositionRange),
    }

    pub struct Button {
        /// Consumed after press, which is fine because the popup menu is then closed
        ty: Option<ButtonType>,
        // TODO support disabled buttons?
    }

    #[derive(Default)]
    struct Buttons(Vec<Button>);

    struct State<'a> {
        single_subject: bool,
        subjects_have_ai: bool,
        subjects_contain_self: bool,
        subjects_are_controllable: bool,
        target_has_path_finding: bool,
        target_is_haulable: bool,
        subjects_are_haulable: bool,

        player_society: Read<'a, PlayerSociety>,
        tile_selection: Read<'a, SelectedTiles>,
        subjects: Write<'a, SelectedEntities>,
    }

    impl<'a> State<'a> {
        fn fetch(world: &'a EcsWorld, ty: PopupContentType) -> Self {
            let (target_entity, _target_pos) = match ty {
                PopupContentType::TileSelection => (None, None),
                PopupContentType::TargetEntity(e) => (Some(e), None),
                PopupContentType::TargetPoint(p) => (None, Some(p)),
            };

            type Query<'a> = (
                Read<'a, SelectedTiles>,
                Write<'a, SelectedEntities>,
                Read<'a, PlayerSociety>,
                ReadStorage<'a, SocietyComponent>,
                ReadStorage<'a, AiComponent>,
                ReadStorage<'a, FollowPathComponent>,
                ReadStorage<'a, HaulableItemComponent>,
                ReadStorage<'a, ContainedInComponent>,
            );
            let (world_sel, entity_sel, player_soc, socs, ais, paths, haulables, containeds) =
                <Query as SystemData>::fetch(world);

            let subjects = entity_sel.iter();

            let has_subjects = !subjects.is_empty();
            let single_subject = subjects.len() == 1;
            let subjects_have_ai = has_subjects && subjects.iter().all(|e| e.has(&ais));
            let subjects_contain_self = has_subjects
                && target_entity
                    .map(|target| subjects.iter().any(|e| *e == target))
                    .unwrap_or_default();
            let subjects_are_controllable = has_subjects
                && subjects
                    .iter()
                    .all(|e| *player_soc == e.get(&socs).map(|comp| comp.handle()));
            let target_has_path_finding = target_entity
                .map(|target| target.has(&paths))
                .unwrap_or_default();

            let is_haulable = |e: Entity| {
                e.has(&haulables)
                    && e.get(&containeds)
                        .map(|comp| {
                            !matches!(
                                comp,
                                ContainedInComponent::Container(_)
                                    | ContainedInComponent::InventoryOf(_)
                            )
                        })
                        .unwrap_or(true)
            };

            let target_is_haulable = target_entity.map(is_haulable).unwrap_or_default();
            let subjects_are_haulable = has_subjects && subjects.iter().copied().all(is_haulable);

            State {
                single_subject,
                subjects_have_ai,
                subjects_contain_self,
                subjects_are_controllable,
                target_has_path_finding,
                target_is_haulable,
                subjects_are_haulable,
                player_society: player_soc,
                tile_selection: world_sel,
                subjects: entity_sel,
            }
        }
    }

    impl State<'_> {
        fn subjects(&self) -> &[Entity] {
            self.subjects.iter()
        }
    }

    #[allow(clippy::collapsible_if)]
    pub fn prepare_popup(ty: PopupContentType, world: &EcsWorld) -> RenderedPopupContent {
        type Query<'a> = (
            Read<'a, WorldRef>,
            ReadStorage<'a, AiComponent>,
            ReadStorage<'a, UiElementComponent>,
        );

        let (voxel_world, ais, uis) = <Query as SystemData>::fetch(world);

        let state = State::fetch(world, ty);
        let mut buttons = Buttons::default();

        // TODO too easy to forget checks here - consider having each declare true/false/ignore needed for every button

        let title;
        match ty {
            PopupContentType::TargetEntity(target_entity) => {
                title = mk_title_for(target_entity, world);

                // follow target entity
                buttons.add(|| {
                    if state.subjects_are_controllable
                        && !state.subjects_contain_self
                        && state.subjects_have_ai
                        && state.target_has_path_finding
                    {
                        return Some(ButtonType::Follow(target_entity));
                    }

                    None
                });

                // cancel divine command
                buttons.add(|| {
                    if state.subjects_are_controllable
                        && state.single_subject
                        && state.subjects_contain_self
                    {
                        if state.subjects()[0] // checked single
                            .get(&ais)
                            .map(|ai| ai.is_current_divine())
                            .unwrap_or_default()
                        {
                            return Some(ButtonType::CancelDivineCommand);
                        }
                    }

                    None
                });

                // haul to tile selection
                buttons.add_multiple(|add| {
                    if state.subjects_are_controllable
                        && state.single_subject
                        && !state.subjects_contain_self
                        && state.target_is_haulable
                    {
                        if let Some(target_pos) = state
                            .tile_selection
                            .current_selected()
                            .and_then(|sel| sel.range().above())
                            .and_then(|range| {
                                voxel_world.borrow().find_accessible_block_in_range(&range)
                            })
                        {
                            // individual
                            let cmd =
                                ButtonCommand::HaulToPosition(target_entity, target_pos.centred());
                            add(ButtonType::Command(None, cmd.clone()));

                            // societal
                            if let soc @ Some(_) = state.player_society.get() {
                                add(ButtonType::Command(soc, cmd));
                            }
                        }
                    }
                });

                // cancel job
                buttons.add(|| {
                    // cancel all selected + target job
                    if state.player_society.has() {
                        let include_target = if state.subjects().contains(&target_entity) {
                            None // dont duplicate
                        } else {
                            Some(target_entity)
                        };

                        let jobs = state
                            .subjects()
                            .iter()
                            .copied()
                            .chain(include_target.into_iter())
                            .filter_map(|e| {
                                e.get(&uis)
                                    .map(|ui| ui.build_job)
                                    .filter(|job| *state.player_society == job.society())
                            })
                            .collect::<SmallVec<[_; 1]>>();

                        if !jobs.is_empty() {
                            return Some(ButtonType::CancelJobs(jobs));
                        }
                    }

                    None
                });
            }
            PopupContentType::TargetPoint(target_pos) => {
                title = format!("{}", target_pos.floor());
                // go to
                buttons.add(|| {
                    if state.subjects_have_ai && state.subjects_are_controllable {
                        return Some(ButtonType::GoTo(target_pos));
                    }

                    None
                });

                // society item haul to here
                buttons.add(|| {
                    if state.subjects_are_haulable {
                        if let Some(soc) = state.player_society.get() {
                            return Some(ButtonType::HaulToTarget(
                                soc,
                                state.subjects().iter().copied().collect(),
                                target_pos,
                            ));
                        }
                    }

                    None
                });
            }
            PopupContentType::TileSelection => {
                title = "Selection".to_owned();

                // break blocks
                if let Some(selection) = state.tile_selection.current_selected() {
                    buttons.add_multiple(|add| {
                        if let soc @ Some(_) = state.player_society.get() {
                            add(ButtonType::Command(
                                soc,
                                ButtonCommand::BreakBlocks(selection.range().clone()),
                            ));
                        }

                        // individual break block
                        if let Some(block) = selection.single_tile() {
                            if state.single_subject
                                && state.subjects_are_controllable
                                && state.subjects_have_ai
                            {
                                add(ButtonType::Command(
                                    None,
                                    ButtonCommand::BreakBlocks(WorldPositionRange::with_single(
                                        block,
                                    )),
                                ))
                            }
                        }
                    });

                    // building
                    buttons.add_multiple(|add| {
                        if let Some(soc) = state.player_society.get() {
                            for (def, template, name) in world.build_templates() {
                                // gross that we need an allocation but the build menu will be different
                                // in the future
                                let name = match name {
                                    Some(s) => Rc::new(s.clone()) as Rc<dyn Display>,
                                    None => Rc::new(*def) as Rc<dyn Display>,
                                };

                                if let Some(range) = selection.range().above() {
                                    let outline = if template.supports_outline()
                                        && range.iter_outline().is_some()
                                    {
                                        Some(ButtonType::Build {
                                            society: soc,
                                            range: range.clone(),
                                            template: template.clone(),
                                            display: name.clone(),
                                            outline_only: true,
                                        })
                                    } else {
                                        None
                                    };

                                    add(ButtonType::Build {
                                        society: soc,
                                        range,
                                        template: template.clone(),
                                        display: name,
                                        outline_only: false,
                                    });

                                    if let Some(outline) = outline {
                                        add(outline)
                                    }
                                }
                            }
                        }
                    });
                }
            }
        }

        RenderedPopupContent {
            title,
            buttons: buttons.0,
        }
    }

    fn mk_title_for(e: Entity, world: &EcsWorld) -> String {
        let name = world.component::<NameComponent>(e);
        let kind = world.component::<KindComponent>(e);

        match (name, kind) {
            (Ok(name), Ok(kind)) => format!("{} ({})", name, kind),
            (Ok(name), Err(_)) => format!("{}", name),
            (Err(_), Ok(kind)) => format!("{}", kind),
            _ => format!("{}", e),
        }
    }

    impl Buttons {
        fn add(&mut self, func: impl FnOnce() -> Option<ButtonType>) {
            if let Some(ty) = func() {
                self.0.push(Button::new(ty))
            }
        }

        fn add_multiple(&mut self, func: impl FnOnce(&mut dyn FnMut(ButtonType))) {
            func(&mut |ty| self.0.push(Button::new(ty)))
        }
    }

    impl Button {
        fn new(ty: ButtonType) -> Self {
            Self { ty: Some(ty) }
        }

        pub fn issue_requests(&mut self, mut issue_req: impl FnMut(UiRequest) -> UiResponse) {
            use ButtonType::*;
            let req = match self.ty.take().expect("reusing consumed button") {
                GoTo(pos) => UiRequest::IssueDivineCommand(AiAction::Goto(pos)),
                Follow(target) => {
                    UiRequest::IssueDivineCommand(AiAction::Follow { target, radius: 3 })
                }
                Command(Some(soc), command) => {
                    // command to player's society
                    let cmd = match command {
                        ButtonCommand::HaulToPosition(e, tgt) => {
                            SocietyCommand::HaulToPosition(e, tgt)
                        }
                        ButtonCommand::BreakBlocks(range) => SocietyCommand::BreakBlocks(range),
                    };

                    UiRequest::IssueSocietyCommand(soc, cmd)
                }
                Command(None, command) => {
                    // divine command to all subjects
                    let divine = match command {
                        ButtonCommand::HaulToPosition(e, tgt) => AiAction::Haul(
                            e,
                            HaulSource::PickUp,
                            HaulTarget::Drop(tgt),
                            HaulPurpose::JustBecause,
                        ),
                        ButtonCommand::BreakBlocks(range) => {
                            let block = range.iter_blocks().next().expect("empty range");
                            AiAction::GoBreakBlock(block)
                        }
                    };

                    UiRequest::IssueDivineCommand(divine)
                }

                HaulToTarget(soc, entities, tgt) => {
                    return for e in entities {
                        issue_req(UiRequest::IssueSocietyCommand(
                            soc,
                            SocietyCommand::HaulToPosition(e, tgt),
                        ));
                    }
                }

                CancelDivineCommand => UiRequest::CancelDivineCommand,
                CancelJobs(jobs) => {
                    return for job in jobs.into_iter() {
                        issue_req(UiRequest::CancelJob(job));
                    }
                }
                Build {
                    society,
                    range,
                    template,
                    outline_only,
                    ..
                } => {
                    if !outline_only {
                        UiRequest::IssueSocietyCommand(
                            society,
                            SocietyCommand::Build(range, template),
                        )
                    } else {
                        return if let Some(outline) = range.iter_outline() {
                            for range in outline {
                                issue_req(UiRequest::IssueSocietyCommand(
                                    society,
                                    SocietyCommand::Build(range, template.clone()),
                                ));
                            }
                        };
                    }
                }
            };

            issue_req(req);
        }
    }

    impl fmt::Display for Button {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Display::fmt(self.ty.as_ref().take().expect("reusing consumed button"), f)
        }
    }

    impl fmt::Display for ButtonType {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            use ButtonCommand::*;
            use ButtonType::*;
            let s = match self {
                GoTo(_) => "Go here",
                Follow(_) => "Follow",
                CancelJobs(jobs) if jobs.len() == 1 => "Cancel job",
                CancelJobs(jobs) => return write!(f, "Cancel {} jobs", jobs.len()),
                CancelDivineCommand => "Cancel divine command",
                Command(soc, cmd) => {
                    // special case
                    let reason = if soc.is_some() {
                        "society"
                    } else {
                        "individual"
                    };
                    let s = match cmd {
                        HaulToPosition(_, _) => "Haul to selection",
                        BreakBlocks(_) => "Break blocks",
                    };

                    return write!(f, "{} ({})", s, reason);
                }
                HaulToTarget(_, entities, _) if entities.len() == 1 => "Haul here",
                HaulToTarget(_, entities, _) => {
                    return write!(f, "Haul {} things here", entities.len())
                }
                Build {
                    display,
                    outline_only,
                    ..
                } => {
                    return write!(
                        f,
                        "Build: {}{}",
                        display,
                        if *outline_only { " (outline)" } else { "" }
                    )
                }
            };
            f.write_str(s)
        }
    }
}
