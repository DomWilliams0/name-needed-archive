use crate::ecs::*;
use crate::event::{EntityEventQueue, RuntimeTimers};
use crate::runtime::Runtime;
use crate::{ActivityComponent, EntityLoggingComponent, Tick};
use common::*;

/// Consumes events, does not run/poll any tasks
pub struct RuntimeSystem;

impl<'a> System<'a> for RuntimeSystem {
    type SystemData = (
        Write<'a, EntityEventQueue>,
        Write<'a, RuntimeTimers>,
        Read<'a, Runtime>,
        WriteStorage<'a, EntityLoggingComponent>,
        WriteStorage<'a, ActivityComponent>,
    );

    fn run(
        &mut self,
        (mut events, mut timers, runtime, mut logging, mut activities): Self::SystemData,
    ) {
        // consume timers
        for (timer_token, task) in timers.maintain(Tick::fetch()) {
            match task.upgrade() {
                Some(task) => {
                    trace!("timer elapsed"; "task" => ?task, "timer" => ?timer_token);
                    runtime.mark_ready(&task);
                }
                None => {
                    trace!("timer elapsed for expired task"; "timer" => ?timer_token);
                }
            }
        }

        // log events
        for (subject, events) in events.events().group_by(|evt| evt.subject).into_iter() {
            let logging = match logging.get_mut(subject.into()) {
                Some(comp) => comp,
                None => continue,
            };

            logging.log_events(events.map(|e| &e.payload));
        }

        #[cfg(feature = "testing")]
        {
            runtime.post_events(events.events().cloned());
        }

        // consume events
        events.consume_events(|subscriber, evt| {
            let task = match activities.get_mut(subscriber.into()) {
                Some(comp) => {
                    if let Some(task) = comp.task() {
                        task
                    } else {
                        warn!("no current task?"; "subscriber" => subscriber); // TODO wut do? task is finished?
                        return false;
                    }
                }
                None => {
                    warn!("subscriber is missing activity component"; "event" => ?evt, "subscriber" => subscriber);
                    return false;
                }
            };

            // event has arrived for task, push it onto task event queue
            task.push_event(evt);

            // mark task as ready now to be polled next tick
            runtime.mark_ready(task);

            true
        });
    }
}
