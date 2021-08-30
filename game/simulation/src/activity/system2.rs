use std::pin::Pin;

use common::*;

use crate::activity::activity2::ActivityContext2;
use crate::activity::NopActivity2;
use crate::ai::{AiAction, AiComponent};
use crate::ecs::*;

use crate::activity::status::{status_channel, StatusReceiver, StatusRef};
use crate::event::EntityEventQueue;
use crate::job::{SocietyJobRef, SocietyTask};
use crate::runtime::{Runtime, TaskHandle, TaskRef, TimerFuture};
use std::cell::Cell;
use std::mem::transmute;
use std::rc::Rc;

// TODO rename
#[derive(Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("activity2")]
pub struct ActivityComponent2 {
    // current: Box<dyn Activity>,
    // current_society_task: Option<(SocietyJobRef, SocietyTask)>,
    /// Set by AI to trigger a new activity
    new_activity: Option<(AiAction, Option<(SocietyJobRef, SocietyTask)>)>,
    current: Option<ActiveTask>,
}

struct ActiveTask {
    task: TaskRef,
    status: StatusReceiver,
    description: Box<dyn Display>,
}

/// Interrupts current with new activities
pub struct ActivitySystem2<'a>(pub Pin<&'a EcsWorld>);

impl Default for ActivityComponent2 {
    fn default() -> Self {
        Self {
            new_activity: None,
            current: None,
        }
    }
}

impl<'a> System<'a> for ActivitySystem2<'a> {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, Runtime>,
        Write<'a, EntityEventQueue>,
        WriteStorage<'a, ActivityComponent2>,
        WriteStorage<'a, AiComponent>,
    );

    fn run(
        &mut self,
        (entities, runtime, mut event_queue, mut activities, mut ais): Self::SystemData,
    ) {
        for (e, activity, ai) in (&entities, &mut activities, &mut ais).join() {
            let e = Entity::from(e);
            let mut new_activity = None;

            if let Some((new_action, new_society_task)) = activity.new_activity.take() {
                // TODO handle society task
                debug!("interrupting activity with new"; e, "action" => ?new_action);

                // cancel current
                if let Some(task) = activity.current.take() {
                    task.task.cancel();

                    // unsubscribe from all events from previous activity
                    event_queue.unsubscribe_all(e);
                }
                // if let Err(e) = activity
                //     .current
                //     .finish(&ActivityFinish::Interrupted, &mut ctx)
                // {
                //     error!("error interrupting current activity"; "activity" => &activity.current, "error" => %e);
                // }

                // comp_updates.remove::<BlockingActivityComponent>(entity);

                // replace current with new activity, dropping the old one
                new_activity = Some(new_action.into_activity2());
            // activity.current = new_action.into_activity2();
            // activity.current_society_task = new_society_task;

            // not necessary to manually cancel society reservation here, as the ai interruption
            // already did
            } else if activity
                .current
                .as_ref()
                .map(|t| t.task.is_finished())
                .unwrap_or(true)
            {
                // current task has finished
                debug!("no activity, reverting to nop"; e);
                new_activity = Some(Rc::new(NopActivity2::default()));

                // TODO interrupt ai and unreserve society task
                // ai.interrupt_current_action(entity, None, || {
                //     entity
                //         .get(&society)
                //         .and_then(|soc| societies.society_by_handle_mut(soc.handle))
                //         .expect("should have society")
                // });

                // next tick ai should return a new decision rather than unchanged to avoid
                // infinite Nop loops
                ai.clear_last_action();

                // TODO notify society job of completion
                // if let Some((job, task)) = activity.current_society_task.take() {
                //     if let Ok(result) = SocietyTaskResult::try_from(finish) {
                //         job.write().notify_completion(task, result);
                //     }
                // }
            }

            // spawn task for new activity
            if let Some(mut new_activity) = new_activity {
                // safety: ecs world is pinned and guaranteed to be valid as long as this system
                // is being ticked
                let world = unsafe { transmute::<Pin<&EcsWorld>, Pin<&'static EcsWorld>>(self.0) };

                let description = new_activity.description();

                // TODO reuse same status updater from previous activity, no need to throw it away
                let (status_tx, status_rx) = status_channel();
                let (taskref_tx, taskref_rx) = futures::channel::oneshot::channel();
                let task = runtime.spawn(taskref_tx, async move {
                    // recv task ref from runtime
                    let task = taskref_rx.await.unwrap(); // will not be cancelled

                    // create context
                    let entity = e.into();
                    let ctx = ActivityContext2::new(
                        entity,
                        world,
                        task,
                        status_tx,
                        new_activity.clone(),
                    );

                    match new_activity.dew_it(ctx).await {
                        Ok(_) => {
                            debug!("activity finished"; entity, "activity" => ?new_activity);
                        }
                        Err(err) => {
                            debug!("activity failed"; entity, "activity" => ?new_activity, "err" => %err);
                        }
                    };
                });

                activity.current = Some(ActiveTask {
                    task,
                    status: status_rx,
                    description,
                });
            }
        }
    }
}

impl ActivityComponent2 {
    pub fn interrupt_with_new_activity(
        &mut self,
        action: AiAction,
        society_task: Option<(SocietyJobRef, SocietyTask)>,
        me: Entity,
        world: &impl ComponentWorld,
    ) {
        self.new_activity = Some((action, society_task));
        // // ensure unblocked
        // world.remove_lazy::<BlockingActivityComponent>(me);
    }

    pub fn task(&self) -> Option<&TaskRef> {
        self.current.as_ref().map(|t| &t.task)
    }

    /// (activity description, current status)
    pub fn status(&self) -> Option<(&dyn Display, StatusRef)> {
        self.current
            .as_ref()
            .map(|t| (&*t.description, t.status.current()))
    }
}
