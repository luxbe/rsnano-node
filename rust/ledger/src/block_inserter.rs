use std::sync::atomic::Ordering;

use rsnano_core::{
    Account, AccountInfo, Amount, BlockEnum, BlockSideband, BlockType, PendingInfo, PendingKey,
};
use rsnano_store_traits::WriteTransaction;

use crate::Ledger;

pub(crate) struct BlockInsertInstructions {
    pub account: Account,
    pub old_account_info: AccountInfo,
    pub new_account_info: AccountInfo,
    pub pending_received: Option<PendingKey>,
    pub new_pending: Option<(PendingKey, PendingInfo)>,
    pub new_sideband: BlockSideband,
    pub is_epoch_block: bool,
}

/// Inserts a new block into the ledger
pub(crate) struct BlockInserter<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut BlockEnum,
    instructions: &'a BlockInsertInstructions,
}

impl<'a> BlockInserter<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut BlockEnum,
        instructions: &'a BlockInsertInstructions,
    ) -> Self {
        Self {
            ledger,
            txn,
            block,
            instructions,
        }
    }

    pub(crate) fn insert(&mut self) {
        self.set_sideband();
        self.ledger.store.block().put(self.txn, self.block);
        self.update_account();
        self.delete_received_pending_entry();
        self.insert_pending_receive();
        self.delete_old_frontier();
        self.insert_frontier();
        self.update_representative_cache();
        self.ledger
            .observer
            .block_added(self.block, self.instructions.is_epoch_block);
        self.ledger.cache.block_count.fetch_add(1, Ordering::SeqCst);
    }

    fn set_sideband(&mut self) {
        self.block
            .set_sideband(self.instructions.new_sideband.clone());
    }

    fn insert_frontier(&mut self) {
        if self.block.block_type() != BlockType::State {
            self.ledger.store.frontier().put(
                self.txn,
                &self.block.hash(),
                &self.instructions.account,
            );
        }
    }

    fn delete_old_frontier(&mut self) {
        if self
            .ledger
            .store
            .frontier()
            .get(self.txn.txn(), &self.instructions.old_account_info.head)
            .is_some()
        {
            self.ledger
                .store
                .frontier()
                .del(self.txn, &self.instructions.old_account_info.head);
        }
    }

    fn insert_pending_receive(&mut self) {
        if let Some((key, info)) = &self.instructions.new_pending {
            self.ledger.store.pending().put(self.txn, key, info);
        }
    }

    fn update_account(&mut self) {
        self.ledger.update_account(
            self.txn,
            &self.instructions.account,
            &self.instructions.old_account_info,
            &self.instructions.new_account_info,
        );
    }

    fn update_representative_cache(&mut self) {
        if !self.instructions.old_account_info.head.is_zero() {
            // Move existing representation & add in amount delta
            self.ledger.cache.rep_weights.representation_add_dual(
                self.instructions.old_account_info.representative,
                Amount::zero().wrapping_sub(self.instructions.old_account_info.balance),
                self.instructions.new_account_info.representative,
                self.instructions.new_account_info.balance,
            );
        } else {
            // Add in amount delta only
            self.ledger.cache.rep_weights.representation_add(
                self.instructions.new_account_info.representative,
                self.instructions.new_account_info.balance,
            );
        }
    }

    fn delete_received_pending_entry(&mut self) {
        if let Some(key) = &self.instructions.pending_received {
            self.ledger.store.pending().del(self.txn, key);
        }
    }
}