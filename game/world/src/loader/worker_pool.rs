use std::time::Duration;

use crossbeam::channel::{unbounded, Receiver, Sender};
use threadpool::ThreadPool;

use common::*;
use unit::world::SlabLocation;

use crate::loader::finalizer::ChunkFinalizer;
use crate::loader::terrain_source::TerrainSourceError;
use crate::loader::LoadedSlab;
use crate::{OcclusionChunkUpdate, WorldRef};
use std::collections::VecDeque;

pub type LoadTerrainResult = Result<LoadedSlab, TerrainSourceError>;

pub trait WorkerPool<D> {
    fn start_finalizer(
        &mut self,
        world: WorldRef<D>,
        finalize_rx: Receiver<LoadTerrainResult>,
        chunk_updates_tx: Sender<OcclusionChunkUpdate>,
    );

    fn block_on_next_finalize(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<SlabLocation, TerrainSourceError>>;

    fn submit<T: 'static + Send + FnOnce() -> LoadTerrainResult>(
        &mut self,
        task: T,
        done_channel: Sender<LoadTerrainResult>,
    );
}

pub struct ThreadedWorkerPool {
    pool: ThreadPool,
    success_rx: Receiver<Result<SlabLocation, TerrainSourceError>>,
    success_tx: Sender<Result<SlabLocation, TerrainSourceError>>,
}

impl ThreadedWorkerPool {
    pub fn new(threads: usize) -> Self {
        let (success_tx, success_rx) = unbounded();
        Self {
            pool: ThreadPool::with_name("wrld-worker".to_owned(), threads),
            success_rx,
            success_tx,
        }
    }
}

impl<D: 'static> WorkerPool<D> for ThreadedWorkerPool {
    fn start_finalizer(
        &mut self,
        world: WorldRef<D>,
        finalize_rx: Receiver<LoadTerrainResult>,
        chunk_updates_tx: Sender<OcclusionChunkUpdate>,
    ) {
        let success_tx = self.success_tx.clone();
        std::thread::Builder::new()
            .name("wrld-finalize".to_owned())
            .spawn(move || {
                let mut finalizer = ChunkFinalizer::new(world, chunk_updates_tx);

                while let Ok(result) = finalize_rx.recv() {
                    let result = match result {
                        Err(e) => {
                            error!("failed to load requested slab"; "error" => %e);
                            Err(e)
                        }
                        Ok(result) => {
                            let slab = result.slab;
                            finalizer.finalize(result);
                            Ok(slab)
                        }
                    };

                    if let Err(e) = success_tx.send(result) {
                        error!("failed to report finalized terrain result"; "error" => %e);
                        trace!("lost result"; "result" => ?e.0);
                    }
                }

                // TODO detect this as an error condition?
                info!("terrain finalizer thread exiting")
            })
            .expect("finalizer thread failed to start");
    }

    fn block_on_next_finalize(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<SlabLocation, TerrainSourceError>> {
        self.success_rx.recv_timeout(timeout).ok()
    }

    fn submit<T: 'static + Send + FnOnce() -> LoadTerrainResult>(
        &mut self,
        task: T,
        done_channel: Sender<LoadTerrainResult>,
    ) {
        self.pool.execute(move || {
            let result = task();

            // terrain has been processed in isolation on worker thread, now post to
            // finalization thread
            if let Err(e) = done_channel.send(result) {
                error!("failed to send terrain result to finalizer"; "error" => %e);
            }
        });
    }
}

#[derive(Default)]
pub struct BlockingWorkerPool<D> {
    finalizer_magic: Option<(Receiver<LoadTerrainResult>, ChunkFinalizer<D>)>,

    #[allow(clippy::type_complexity)]
    task_queue: VecDeque<(
        Box<dyn FnOnce() -> LoadTerrainResult>,
        Sender<LoadTerrainResult>,
    )>,
}

impl<D> WorkerPool<D> for BlockingWorkerPool<D> {
    fn start_finalizer(
        &mut self,
        world: WorldRef<D>,
        finalize_rx: Receiver<LoadTerrainResult>,
        chunk_updates_tx: Sender<OcclusionChunkUpdate>,
    ) {
        self.finalizer_magic = Some((finalize_rx, ChunkFinalizer::new(world, chunk_updates_tx)));
    }

    fn block_on_next_finalize(
        &mut self,
        _: Duration,
    ) -> Option<Result<SlabLocation, TerrainSourceError>> {
        // time to actually do the work
        let (task, done_channel) = self.task_queue.pop_front()?;

        let (finalize_rx, finalizer) = self.finalizer_magic.as_mut().unwrap(); // set in start_finalizer

        // load chunk right here right now
        let result = task();

        // post to "finalizer thread"
        done_channel
            .send(result)
            .expect("failed to send to finalizer");

        // receive on "finalizer thread"
        let result = match finalize_rx
            .recv_timeout(Duration::from_secs(60))
            .expect("expected finalized terrain by now")
        {
            Err(e) => {
                error!("failed to load chunk"; "error" => %e);
                Err(e)
            }
            Ok(result) => {
                let chunk = result.slab;

                // finalize on "finalizer thread"
                finalizer.finalize(result);
                Ok(chunk)
            }
        };

        // send back to "main thread"
        Some(result)
    }

    fn submit<T: 'static + Send + FnOnce() -> LoadTerrainResult>(
        &mut self,
        task: T,
        done_channel: Sender<LoadTerrainResult>,
    ) {
        // naaah, do the work later when we're asked for it
        self.task_queue.push_back((Box::new(task), done_channel));
    }
}
