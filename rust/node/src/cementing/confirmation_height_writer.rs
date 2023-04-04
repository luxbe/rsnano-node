use std::{
    cmp::max,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::Context;
use rsnano_core::{
    utils::Logger, BlockEnum, BlockHash, ConfirmationHeightInfo, UpdateConfirmationHeight,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::{Transaction, WriteTransaction};

use crate::stats::{DetailType, Direction, StatType, Stats};

use super::{
    accounts_confirmed_map::AccountsConfirmedMap,
    write_details_queue::{WriteDetails, WriteDetailsQueue},
    UpdateConfirmationHeightCommandFactory,
};

pub(crate) struct ConfirmationHeightWriter<'a> {
    pub cemented_batch_timer: Instant,
    pub pending_writes: &'a mut WriteDetailsQueue,
    ledger: &'a Ledger,
    stats: &'a Stats,
    batch_write_size: &'a AtomicUsize,
    write_database_queue: &'a WriteDatabaseQueue,

    /// Will contain all blocks that have been cemented (bounded by batch_write_size)
    /// and will get run through the cemented observer callback
    pub cemented_blocks: Vec<Arc<BlockEnum>>,

    logger: &'a dyn Logger,
    enable_timing_logging: bool,
    accounts_confirmed_info: &'a mut AccountsConfirmedMap,
    scoped_write_guard: &'a mut WriteGuard,
    block_cemented: &'a mut dyn FnMut(&Arc<BlockEnum>),
    batch_size_amount_to_change: usize,
    pending: WriteDetails,
    confirmation_height_info: ConfirmationHeightInfo,
}

impl<'a> ConfirmationHeightWriter<'a> {
    pub(crate) const MINIMUM_BATCH_WRITE_SIZE: usize = 16384;
    pub(crate) const MAXIMUM_BATCH_WRITE_TIME: Duration = Duration::from_millis(250);

    pub(crate) const MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF: Duration =
        eighty_percent_of(Self::MAXIMUM_BATCH_WRITE_TIME);

    pub fn new(
        pending_writes: &'a mut WriteDetailsQueue,
        ledger: &'a Ledger,
        stats: &'a Stats,
        batch_write_size: &'a AtomicUsize,
        write_database_queue: &'a WriteDatabaseQueue,
        logger: &'a dyn Logger,
        enable_timing_logging: bool,
        accounts_confirmed_info: &'a mut AccountsConfirmedMap,
        scoped_write_guard: &'a mut WriteGuard,
        block_cemented: &'a mut dyn FnMut(&Arc<BlockEnum>),
    ) -> Self {
        Self {
            cemented_batch_timer: Instant::now(),
            pending_writes,
            ledger,
            stats,
            batch_size_amount_to_change: batch_write_size.load(Ordering::SeqCst) / 10,
            batch_write_size,
            write_database_queue,
            cemented_blocks: Vec::new(),
            logger,
            enable_timing_logging,
            accounts_confirmed_info,
            scoped_write_guard,
            block_cemented,
            pending: Default::default(),
            confirmation_height_info: Default::default(),
        }
    }

    pub(crate) fn write(&mut self) {
        // This only writes to the confirmation_height table and is the only place to do so in a single process
        let mut txn = self.ledger.store.tx_begin_write();
        self.reset_batch_timer();

        // Cement all pending entries, each entry is specific to an account and contains the least amount
        // of blocks to retain consistent cementing across all account chains to genesis.
        while let Some(pending) = self.pending_writes.pop_front() {
            self.cement_pending_block(txn.as_mut(), pending.clone());

            if let Some(found_info) = self.accounts_confirmed_info.get(&pending.account) {
                if found_info.confirmed_height == pending.top_height {
                    self.accounts_confirmed_info.remove(&self.pending.account);
                }
            }
        }

        drop(txn);

        let time_spent_cementing = self.cemented_batch_timer.elapsed();
        if time_spent_cementing > Duration::from_millis(50) {
            self.log_cemented_blocks(time_spent_cementing);
        }

        // Scope guard could have been released earlier (0 cemented_blocks would indicate that)
        if self.scoped_write_guard.is_owned() && !self.cemented_blocks.is_empty() {
            self.scoped_write_guard.release();
            self.publish_cemented_blocks();
        }

        if time_spent_cementing > ConfirmationHeightWriter::MAXIMUM_BATCH_WRITE_TIME {
            self.reduce_batch_write_size();
        }
        debug_assert!(self.pending_writes.is_empty());
    }

    fn load_block_callback<'b>(
        ledger: &'b Ledger,
        txn: &'b dyn Transaction,
    ) -> impl Fn(BlockHash) -> Option<BlockEnum> + 'b {
        |block_hash| ledger.store.block().get(txn, &block_hash)
    }

    fn cement_pending_block(&mut self, txn: &mut dyn WriteTransaction, pending: WriteDetails) {
        self.pending = pending.clone();
        let confirmation_height_info = self
            .ledger
            .store
            .confirmation_height()
            .get(txn.txn(), &self.pending.account)
            .unwrap_or_default();

        self.confirmation_height_info = confirmation_height_info.clone();

        let mut update_command_factory = UpdateConfirmationHeightCommandFactory::new(
            &pending,
            &confirmation_height_info,
            &self.batch_write_size,
        );

        loop {
            let load_block = Self::load_block_callback(&self.ledger, txn.txn());
            if let Some(update_command) = update_command_factory
                .create_command(&load_block, &mut self.cemented_blocks)
                .with_context(|| {
                    format!(
                        "Could not create update confirmation height command for account {}",
                        pending.account
                    )
                })
                .unwrap()
            {
                drop(load_block);
                self.flush(txn, &update_command, &update_command_factory);
            } else {
                break;
            }
        }
    }

    fn flush(
        &mut self,
        txn: &mut dyn WriteTransaction,
        update_command: &UpdateConfirmationHeight,
        update_command_factory: &UpdateConfirmationHeightCommandFactory,
    ) {
        self.write_confirmation_height(txn, update_command);
        let time_spent_cementing = self.cemented_batch_timer.elapsed();
        txn.commit();

        self.log_cemented_blocks(time_spent_cementing);
        self.adjust_batch_write_size(time_spent_cementing);
        self.scoped_write_guard.release();
        self.publish_cemented_blocks();

        // Only aquire transaction if there are blocks left
        if self.is_another_flush_needed(&update_command_factory) {
            *self.scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            txn.renew();
        }

        self.reset_batch_timer();
    }

    fn is_another_flush_needed(
        &self,
        update_command_factory: &UpdateConfirmationHeightCommandFactory,
    ) -> bool {
        !update_command_factory.is_done() || self.pending_writes.len() > 0
    }

    fn publish_cemented_blocks(&mut self) {
        for block in &self.cemented_blocks {
            (self.block_cemented)(block);
        }

        self.cemented_blocks.clear();
    }

    fn adjust_batch_write_size(&self, time_spent_cementing: Duration) {
        // Update the maximum amount of blocks to write next time based on the time it took to cement this batch.
        if time_spent_cementing > Self::MAXIMUM_BATCH_WRITE_TIME {
            self.reduce_batch_write_size();
        } else if time_spent_cementing < Self::MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF {
            // Increase amount of blocks written for next batch if the time for writing this one is sufficiently lower than the max time to warrant changing
            self.increase_batch_write_size();
        }
    }

    fn log_cemented_blocks(&self, time_spent_cementing: Duration) {
        if self.enable_timing_logging {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms (bounded processor)",
                self.cemented_blocks.len(),
                time_spent_cementing.as_millis()
            ));
        }
    }

    pub fn reset_batch_timer(&mut self) {
        self.cemented_batch_timer = Instant::now();
    }

    pub fn write_confirmation_height(
        &self,
        txn: &mut dyn WriteTransaction,
        update_confirmation_height: &UpdateConfirmationHeight,
    ) {
        self.ledger
            .write_confirmation_height(txn, &update_confirmation_height);

        self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmedBounded,
            Direction::In,
            update_confirmation_height.num_blocks_cemented,
            false,
        );
    }

    pub fn increase_batch_write_size(&self) {
        self.batch_write_size
            .fetch_add(self.batch_size_amount_to_change, Ordering::SeqCst);
    }

    pub fn reduce_batch_write_size(&self) {
        // Reduce (unless we have hit a floor)
        self.batch_write_size.store(
            max(
                ConfirmationHeightWriter::MINIMUM_BATCH_WRITE_SIZE,
                self.batch_write_size.load(Ordering::SeqCst) - self.batch_size_amount_to_change,
            ),
            Ordering::SeqCst,
        );
    }
}

const fn eighty_percent_of(d: Duration) -> Duration {
    let millis = d.as_millis() as u64;
    Duration::from_millis(millis - (millis / 5))
}
