use std::any::Any;
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use common::*;
use unit::world::WorldPoint;

use crate::build::BuildTemplate;
use crate::job::list::SocietyJobHandle;
use crate::job::SocietyTask;
use crate::society::Society;
use crate::{EcsWorld, Entity, WorldPositionRange};

/// A high-level society job that produces a number of [SocietyTask]s. Unsized but it lives in an
/// Rc anyway
pub struct SocietyJob<J: ?Sized = dyn SocietyJobImpl> {
    /// Tasks still in progress
    tasks: Vec<SocietyTask>,

    pending_complete: SmallVec<[(SocietyTask, SocietyTaskResult); 1]>,

    inner: J,
}

#[derive(Debug)]
pub enum SocietyTaskResult {
    Success,
    Failure(Box<dyn Error>),
}

#[derive(Clone)]
pub struct SocietyJobRef(Rc<RefCell<SocietyJob>>, SocietyJobHandle);

pub(in crate::society::job) type CompletedTasks<'a> = &'a mut [(SocietyTask, SocietyTaskResult)];

pub trait SocietyJobImpl: Display + Debug {
    /// [refresh_tasks] will be called after this before any tasks are dished out, so this can eagerly add
    /// tasks without filtering.
    ///
    /// TODO provide size hint that could be used as an optimisation for a small number of tasks (e.g. smallvec)
    fn populate_initial_tasks(
        &mut self,
        world: &EcsWorld,
        out: &mut Vec<SocietyTask>,
        this_job: SocietyJobHandle,
    );

    /// Update `tasks` and apply `completions`. Completions are considered owned by this method, as
    /// the underlying container will be cleared on return, so feel free to move results out.
    /// Return None if ongoing.
    /// If 0 tasks are left this counts as a success.
    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: CompletedTasks,
    ) -> Option<SocietyTaskResult>;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Declarative society command that maps to a [SocietyJob]
#[derive(Debug)]
pub enum SocietyCommand {
    BreakBlocks(WorldPositionRange),
    Build(WorldPositionRange, Rc<BuildTemplate>),
    HaulToPosition(Entity, WorldPoint),

    /// (thing, container)
    HaulIntoContainer(Entity, Entity),
}

impl SocietyCommand {
    pub fn submit_job_to_society(self, society: &Society, world: &EcsWorld) -> Result<(), Self> {
        use self::SocietyCommand::*;
        use crate::job::jobs::*;

        let mut jobs = society.jobs_mut();
        let jobs = jobs.borrow_mut();

        macro_rules! job {
            ($job:expr) => {
                jobs.submit(world, $job)
            };
        }

        match self {
            BreakBlocks(range) => job!(BreakBlocksJob::new(range)),
            Build(pos, template) => {
                for block in pos.iter_blocks() {
                    jobs.submit(world, BuildThingJob::new(block, template.clone()))
                }
            }
            HaulToPosition(e, pos) => {
                job!(HaulJob::with_target_position(e, pos, world).ok_or(self)?)
            }
            HaulIntoContainer(e, container) => {
                job!(HaulJob::with_target_container(e, container, world).ok_or(self)?)
            }
        }

        Ok(())
    }
}

impl SocietyJob<dyn SocietyJobImpl> {
    pub(in crate::society) fn create(
        world: &EcsWorld,
        handle: SocietyJobHandle,
        mut job: impl SocietyJobImpl + 'static,
    ) -> SocietyJobRef {
        let mut tasks = Vec::new();
        job.populate_initial_tasks(world, &mut tasks, handle);

        SocietyJobRef(
            Rc::new(RefCell::new(SocietyJob {
                tasks,
                pending_complete: SmallVec::new(),
                inner: job,
            })),
            handle,
        )
    }

    pub(in crate::society::job) fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
    ) -> Option<SocietyTaskResult> {
        let ret = self
            .inner
            .refresh_tasks(world, &mut self.tasks, &mut self.pending_complete)
            .or(if self.tasks.is_empty() {
                // no tasks left and no specific result returned
                Some(SocietyTaskResult::Success)
            } else {
                None
            });

        self.pending_complete.clear();
        ret
    }

    pub fn tasks(&self) -> impl Iterator<Item = &SocietyTask> + '_ {
        self.tasks.iter()
    }

    pub fn notify_completion(&mut self, task: SocietyTask, result: SocietyTaskResult) {
        self.pending_complete.push((task, result));
    }

    pub fn cast<J: SocietyJobImpl + 'static>(&self) -> Option<&J> {
        self.inner.as_any().downcast_ref()
    }

    pub fn cast_mut<J: SocietyJobImpl + 'static>(&mut self) -> Option<&mut J> {
        self.inner.as_any_mut().downcast_mut()
    }
}

impl SocietyJobRef {
    pub fn handle(&self) -> SocietyJobHandle {
        self.1
    }
}

impl From<BoxedResult<()>> for SocietyTaskResult {
    fn from(result: BoxedResult<()>) -> Self {
        match result {
            Ok(_) => Self::Success,
            Err(err) => Self::Failure(err),
        }
    }
}

impl Debug for SocietyJobRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "SocietyJob(")?;

        match self.0.try_borrow() {
            Err(_) => write!(f, "<locked>)"),
            Ok(job) => write!(
                f,
                "{:?} | {} tasks: {:?})",
                &job.inner,
                job.tasks.len(),
                job.tasks
            ),
        }
    }
}

impl Deref for SocietyJobRef {
    type Target = Rc<RefCell<SocietyJob>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for SocietyJobRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.0.try_borrow() {
            Err(_) => write!(f, "<locked>)"),
            Ok(job) => Display::fmt(&job.inner, f),
        }
    }
}
