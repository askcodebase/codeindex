use std::cmp::min;
use std::collections::HashSet;
use std::sync::Arc;

use itertools::Itertools;
use log::{debug, error, info, trace, warn};
use segment::entry::entry_point::OperationResult;
use segment::types::SeqNumberType;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::{oneshot, Mutex as TokioMutex};
use tokio::task::JoinHandle;
use tokio::time::Duration;

use crate::collection_manager::collection_updater::CollectionUpdater;
use crate::collection_manager::holders::segment_holder::LockedSegmentHolder;
use crate::collection_manager::optimizers::segment_optimizer::SegmentOptimizer;
use crate::common::stoppable_task::{spawn_stoppable, StoppableTaskHandle};
use crate::operations::shared_storage_config::SharedStorageConfig;
use crate::operations::types::{CollectionError, CollectionResult};
use crate::operations::CollectionUpdateOperations;
use crate::shards::local_shard::LockedWal;
use crate::wal::WalError;

pub type Optimizer = dyn SegmentOptimizer + Sync + Send;

/// Information, required to perform operation and notify regarding the result
#[derive(Debug)]
pub struct OperationData {
    /// Sequential number of the operation
    pub op_num: SeqNumberType,
    /// Operation
    pub operation: CollectionUpdateOperations,
    /// If operation was requested to wait for result
    pub wait: bool,
    /// Callback notification channel
    pub sender: Option<oneshot::Sender<CollectionResult<usize>>>,
}

/// Signal, used to inform Updater process
#[derive(Debug)]
pub enum UpdateSignal {
    /// Requested operation to perform
    Operation(OperationData),
    /// Stop all optimizers and listening
    Stop,
    /// Empty signal used to trigger optimizers
    Nop,
    /// Ensures that previous updates are applied
    Plunger(oneshot::Sender<()>),
}

/// Signal, used to inform Optimization process
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum OptimizerSignal {
    /// Sequential number of the operation
    Operation(SeqNumberType),
    /// Stop all optimizers and listening
    Stop,
    /// Empty signal used to trigger optimizers
    Nop,
}

/// Structure, which holds object, required for processing updates of the collection
pub struct UpdateHandler {
    shared_storage_config: Arc<SharedStorageConfig>,
    /// List of used optimizers
    pub optimizers: Arc<Vec<Arc<Optimizer>>>,
    /// How frequent can we flush data
    pub flush_interval_sec: u64,
    segments: LockedSegmentHolder,
    /// Process, that listens updates signals and perform updates
    update_worker: Option<JoinHandle<()>>,
    /// Process, that listens for post-update signals and performs optimization
    optimizer_worker: Option<JoinHandle<()>>,
    /// Process that periodically flushes segments and tries to truncate wal
    flush_worker: Option<JoinHandle<()>>,
    /// Sender to stop flush worker
    flush_stop: Option<oneshot::Sender<()>>,
    runtime_handle: Handle,
    /// WAL, required for operations
    wal: LockedWal,
    optimization_handles: Arc<TokioMutex<Vec<StoppableTaskHandle<bool>>>>,
    max_optimization_threads: usize,
}

impl UpdateHandler {
    pub fn new(
        shared_storage_config: Arc<SharedStorageConfig>,
        optimizers: Arc<Vec<Arc<Optimizer>>>,
        runtime_handle: Handle,
        segments: LockedSegmentHolder,
        wal: LockedWal,
        flush_interval_sec: u64,
        max_optimization_threads: usize,
    ) -> UpdateHandler {
        UpdateHandler {
            shared_storage_config,
            optimizers,
            segments,
            update_worker: None,
            optimizer_worker: None,
            flush_worker: None,
            flush_stop: None,
            runtime_handle,
            wal,
            flush_interval_sec,
            optimization_handles: Arc::new(TokioMutex::new(vec![])),
            max_optimization_threads,
        }
    }

    pub fn run_workers(&mut self, update_receiver: Receiver<UpdateSignal>) {
        let (tx, rx) = mpsc::channel(self.shared_storage_config.update_queue_size);
        self.optimizer_worker = Some(self.runtime_handle.spawn(Self::optimization_worker_fn(
            self.optimizers.clone(),
            tx.clone(),
            rx,
            self.segments.clone(),
            self.wal.clone(),
            self.optimization_handles.clone(),
            self.max_optimization_threads,
        )));
        self.update_worker = Some(self.runtime_handle.spawn(Self::update_worker_fn(
            update_receiver,
            tx,
            self.wal.clone(),
            self.segments.clone(),
        )));
        let (flush_tx, flush_rx) = oneshot::channel();
        self.flush_worker = Some(self.runtime_handle.spawn(Self::flush_worker(
            self.segments.clone(),
            self.wal.clone(),
            self.flush_interval_sec,
            flush_rx,
        )));
        self.flush_stop = Some(flush_tx);
    }

    pub fn stop_flush_worker(&mut self) {
        if let Some(flush_stop) = self.flush_stop.take() {
            if let Err(()) = flush_stop.send(()) {
                warn!("Failed to stop flush worker as it is already stopped.");
            }
        }
    }

    /// Gracefully wait before all optimizations stop
    /// If some optimization is in progress - it will be finished before shutdown.
    pub async fn wait_workers_stops(&mut self) -> CollectionResult<()> {
        let maybe_handle = self.update_worker.take();
        if let Some(handle) = maybe_handle {
            handle.await?;
        }
        let maybe_handle = self.optimizer_worker.take();
        if let Some(handle) = maybe_handle {
            handle.await?;
        }
        let maybe_handle = self.flush_worker.take();
        if let Some(handle) = maybe_handle {
            handle.await?;
        }

        let mut opt_handles_guard = self.optimization_handles.lock().await;
        let opt_handles = std::mem::take(&mut *opt_handles_guard);
        let stopping_handles = opt_handles
            .into_iter()
            .filter_map(|h| h.stop())
            .collect_vec();

        for res in stopping_handles {
            res.await?;
        }

        Ok(())
    }

    /// Checks if there are any failed operations.
    /// If so - attempts to re-apply all failed operations.
    async fn try_recover(segments: LockedSegmentHolder, wal: LockedWal) -> CollectionResult<usize> {
        // Try to re-apply everything starting from the first failed operation
        let first_failed_operation_option = segments.read().failed_operation.iter().cloned().min();
        match first_failed_operation_option {
            None => {}
            Some(first_failed_op) => {
                let wal_lock = wal.lock();
                for (op_num, operation) in wal_lock.read(first_failed_op) {
                    CollectionUpdater::update(&segments, op_num, operation)?;
                }
            }
        };
        Ok(0)
    }

    /// Checks conditions for all optimizers until there is no suggested segment
    /// Starts a task for each optimization
    /// Returns handles for started tasks
    pub(crate) fn launch_optimization<F>(
        optimizers: Arc<Vec<Arc<Optimizer>>>,
        segments: LockedSegmentHolder,
        callback: F,
    ) -> Vec<StoppableTaskHandle<bool>>
    where
        F: FnOnce(bool),
        F: Send + 'static,
        F: Clone,
    {
        let mut scheduled_segment_ids: HashSet<_> = Default::default();
        let mut handles = vec![];
        for optimizer in optimizers.iter() {
            loop {
                let nonoptimal_segment_ids =
                    optimizer.check_condition(segments.clone(), &scheduled_segment_ids);
                if nonoptimal_segment_ids.is_empty() {
                    break;
                } else {
                    let optim = optimizer.clone();
                    let segs = segments.clone();
                    let nsi = nonoptimal_segment_ids.clone();
                    for sid in &nsi {
                        scheduled_segment_ids.insert(*sid);
                    }
                    let callback_cloned = callback.clone();

                    handles.push(spawn_stoppable(move |stopped| {
                        match optim.as_ref().optimize(segs.clone(), nsi, stopped) {
                            Ok(result) => {
                                callback_cloned(result); // Perform some actions when optimization if finished
                                result
                            }
                            Err(error) => match error {
                                CollectionError::Cancelled { description } => {
                                    log::debug!("Optimization cancelled - {}", description);
                                    false
                                }
                                _ => {
                                    // Save only the first error
                                    // If is more likely to be the real cause of all further problems
                                    segs.write().report_optimizer_error(error.clone());

                                    // Error of the optimization can not be handled by API user
                                    // It is only possible to fix after full restart,
                                    // so the best available action here is to stop whole
                                    // optimization thread and log the error
                                    log::error!("Optimization error: {}", error);
                                    panic!("Optimization error: {error}");
                                }
                            },
                        }
                    }));
                }
            }
        }
        handles
    }

    pub(crate) async fn process_optimization(
        optimizers: Arc<Vec<Arc<Optimizer>>>,
        segments: LockedSegmentHolder,
        optimization_handles: Arc<TokioMutex<Vec<StoppableTaskHandle<bool>>>>,
        sender: Sender<OptimizerSignal>,
    ) {
        let mut new_handles = Self::launch_optimization(
            optimizers.clone(),
            segments.clone(),
            move |_optimization_result| {
                // After optimization is finished, we still need to check if there are
                // some further optimizations possible.
                // If receiver is already dead - we do not care.
                // If channel is full - optimization will be triggered by some other signal
                let _ = sender.try_send(OptimizerSignal::Nop);
            },
        );
        let mut handles = optimization_handles.lock().await;
        handles.append(&mut new_handles);
        handles.retain(|h| !h.is_finished())
    }

    async fn optimization_worker_fn(
        optimizers: Arc<Vec<Arc<Optimizer>>>,
        sender: Sender<OptimizerSignal>,
        mut receiver: Receiver<OptimizerSignal>,
        segments: LockedSegmentHolder,
        wal: LockedWal,
        optimization_handles: Arc<TokioMutex<Vec<StoppableTaskHandle<bool>>>>,
        max_handles: usize,
    ) {
        while let Some(signal) = receiver.recv().await {
            match signal {
                OptimizerSignal::Nop | OptimizerSignal::Operation(_) => {
                    if signal != OptimizerSignal::Nop
                        && optimization_handles.lock().await.len() >= max_handles
                    {
                        let mut handles = optimization_handles.lock().await;
                        handles.retain(|h| !h.is_finished());
                        continue;
                    }
                    // We skip the check for number of optimization handles here
                    // Because `Nop` usually means that we need to force the optimization
                    if Self::try_recover(segments.clone(), wal.clone())
                        .await
                        .is_err()
                    {
                        continue;
                    }
                    Self::process_optimization(
                        optimizers.clone(),
                        segments.clone(),
                        optimization_handles.clone(),
                        sender.clone(),
                    )
                    .await;
                }

                OptimizerSignal::Stop => break,
            }
        }
    }

    async fn update_worker_fn(
        mut receiver: Receiver<UpdateSignal>,
        optimize_sender: Sender<OptimizerSignal>,
        wal: LockedWal,
        segments: LockedSegmentHolder,
    ) {
        while let Some(signal) = receiver.recv().await {
            match signal {
                UpdateSignal::Operation(OperationData {
                    op_num,
                    operation,
                    sender,
                    wait,
                }) => {
                    let flush_res = if wait {
                        wal.lock().flush().map_err(|err| {
                            CollectionError::service_error(format!(
                                "Can't flush WAL before operation {} - {}",
                                op_num, err
                            ))
                        })
                    } else {
                        Ok(())
                    };

                    let operation_result = flush_res
                        .and_then(|_| CollectionUpdater::update(&segments, op_num, operation));

                    let res = match operation_result {
                        Ok(update_res) => optimize_sender
                            .send(OptimizerSignal::Operation(op_num))
                            .await
                            .and(Ok(update_res))
                            .map_err(|send_err| send_err.into()),
                        Err(err) => Err(err),
                    };

                    if let Some(feedback) = sender {
                        feedback.send(res).unwrap_or_else(|_| {
                            info!(
                                "Can't report operation {} result. Assume already not required",
                                op_num
                            );
                        });
                    };
                }
                UpdateSignal::Stop => {
                    optimize_sender
                        .send(OptimizerSignal::Stop)
                        .await
                        .unwrap_or_else(|_| debug!("Optimizer already stopped"));
                    break;
                }
                UpdateSignal::Nop => optimize_sender
                    .send(OptimizerSignal::Nop)
                    .await
                    .unwrap_or_else(|_| {
                        info!(
                            "Can't notify optimizers, assume process is dead. Restart is required"
                        );
                    }),
                UpdateSignal::Plunger(callback_sender) => {
                    callback_sender.send(()).unwrap_or_else(|_| {
                        debug!("Can't notify sender, assume nobody is waiting anymore");
                    });
                }
            }
        }
        // Transmitter was destroyed
        optimize_sender
            .send(OptimizerSignal::Stop)
            .await
            .unwrap_or_else(|_| debug!("Optimizer already stopped"));
    }

    async fn flush_worker(
        segments: LockedSegmentHolder,
        wal: LockedWal,
        flush_interval_sec: u64,
        mut stop_receiver: oneshot::Receiver<()>,
    ) {
        loop {
            // Stop flush worker on signal or if sender was dropped
            // Even if timer did not finish
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(flush_interval_sec)) => {},
                _ = &mut stop_receiver => {
                    debug!("Stopping flush worker.");
                    return;
                }
            };

            trace!("Attempting flushing");
            let wal_flash_job = wal.lock().flush_async();

            if let Err(err) = wal_flash_job.join() {
                error!("Failed to flush wal: {:?}", err);
                segments
                    .write()
                    .report_optimizer_error(WalError::WriteWalError(format!(
                        "WAL flush error: {err:?}"
                    )));
                continue;
            }

            let confirmed_version = Self::flush_segments(segments.clone());
            let confirmed_version = match confirmed_version {
                Ok(version) => version,
                Err(err) => {
                    error!("Failed to flush: {err}");
                    segments.write().report_optimizer_error(err);
                    continue;
                }
            };
            if let Err(err) = wal.lock().ack(confirmed_version) {
                segments.write().report_optimizer_error(err);
            }
        }
    }

    /// Returns confirmed version after flush of all segments
    ///
    /// # Errors
    /// Returns an error on flush failure
    fn flush_segments(segments: LockedSegmentHolder) -> OperationResult<SeqNumberType> {
        let read_segments = segments.read();
        let flushed_version = read_segments.flush_all(false)?;
        Ok(match read_segments.failed_operation.iter().cloned().min() {
            None => flushed_version,
            Some(failed_operation) => min(failed_operation, flushed_version),
        })
    }
}
