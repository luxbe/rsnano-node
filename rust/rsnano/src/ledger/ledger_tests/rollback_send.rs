use std::sync::atomic::Ordering;

use crate::{
    core::{Account, Amount, Block, PendingKey},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::{LedgerWithOpenBlock, LedgerWithSendBlock};

#[test]
fn update_vote_weight() {
    let mut ctx = LedgerWithSendBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger().weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
}

#[test]
fn update_frontier_store() {
    let mut ctx = LedgerWithSendBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &DEV_GENESIS_HASH),
        *DEV_GENESIS_ACCOUNT
    );
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.send_block.hash()),
        Account::zero()
    );
}

#[test]
fn update_account_store() {
    let mut ctx = LedgerWithSendBlock::new();

    ctx.rollback();

    let account_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(account_info.block_count, 1);
    assert_eq!(account_info.head, *DEV_GENESIS_HASH);
    assert_eq!(account_info.balance, DEV_CONSTANTS.genesis_amount);
    assert_eq!(ctx.ledger().cache.account_count.load(Ordering::Relaxed), 1);
}

#[test]
fn remove_from_pending_store() {
    let mut ctx = LedgerWithSendBlock::new();

    ctx.rollback();

    let pending = ctx.ledger().store.pending().get(
        ctx.txn.txn(),
        &PendingKey::new(ctx.receiver_key.public_key().into(), ctx.send_block.hash()),
    );
    assert_eq!(pending, None);
}

#[test]
fn update_confirmation_height_store() {
    let mut ctx = LedgerWithSendBlock::new();

    ctx.rollback();

    let conf_height = ctx
        .ledger()
        .store
        .confirmation_height()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(conf_height.frontier, *DEV_GENESIS_HASH);
    assert_eq!(conf_height.height, 1);
}

#[test]
fn rollback_dependent_blocks_too() {
    let mut ctx = LedgerWithOpenBlock::new();

    // Rollback send block. This requires the rollback of the open block first.
    ctx.ledger_context
        .ledger
        .rollback(ctx.txn.as_mut(), &ctx.send_block.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        DEV_CONSTANTS.genesis_amount
    );

    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &ctx.receiver_account, false),
        Amount::zero()
    );

    assert!(ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &ctx.receiver_account)
        .is_none());

    let pending = ctx.ledger().store.pending().get(
        ctx.txn.txn(),
        &PendingKey::new(ctx.receiver_account, *DEV_GENESIS_HASH),
    );
    assert_eq!(pending, None);
}