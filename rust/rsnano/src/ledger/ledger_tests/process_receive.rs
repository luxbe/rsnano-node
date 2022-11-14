use crate::{
    core::{Account, Amount, Block, BlockBuilder, BlockEnum},
    ledger::{ledger_tests::LedgerWithReceiveBlock, ProcessResult},
    DEV_GENESIS_ACCOUNT,
};

use super::LedgerWithOpenBlock;

#[test]
fn update_sideband() {
    let ctx = LedgerWithReceiveBlock::new();
    let sideband = ctx.receive_block.sideband().unwrap();
    assert_eq!(sideband.account, ctx.receiver_account);
    assert_eq!(sideband.balance, ctx.expected_receiver_balance);
    assert_eq!(sideband.height, 2);
}

#[test]
fn save_block() {
    let ctx = LedgerWithReceiveBlock::new();

    let loaded_block = ctx
        .ledger()
        .store
        .block()
        .get(ctx.txn.txn(), &ctx.receive_block.hash())
        .unwrap();

    let BlockEnum::Receive(loaded_block) = loaded_block else{panic!("not a receive block")};
    assert_eq!(loaded_block, ctx.receive_block);
    assert_eq!(
        loaded_block.sideband().unwrap(),
        ctx.receive_block.sideband().unwrap()
    );
}

#[test]
fn update_block_amount() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger()
            .amount(ctx.txn.txn(), &ctx.receive_block.hash()),
        Some(Amount::new(25))
    );
}

#[test]
fn update_frontier_store() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.open_block.hash()),
        Account::zero()
    );
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.receive_block.hash()),
        ctx.receiver_account
    );
}

#[test]
fn update_balance() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        Amount::new(25)
    );
    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &ctx.receiver_account, false),
        ctx.expected_receiver_balance
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger().weight(&ctx.receiver_account),
        ctx.expected_receiver_balance
    );
    assert_eq!(ctx.ledger().weight(&DEV_GENESIS_ACCOUNT), Amount::new(25));
}

#[test]
fn update_account_receivable() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_receivable(ctx.txn.txn(), &ctx.receiver_account, false),
        Amount::zero()
    );
}

#[test]
fn update_latest_block() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger().latest(ctx.txn.txn(), &ctx.receiver_account),
        Some(ctx.receive_block.hash())
    );
}

#[test]
fn receive_fork() {
    let mut ctx = LedgerWithOpenBlock::new();

    let send = ctx.ledger_context.process_send_from_genesis(
        ctx.txn.as_mut(),
        &ctx.receiver_account,
        Amount::new(1),
    );

    ctx.ledger_context
        .process_change(ctx.txn.as_mut(), &ctx.receiver_key, Account::from(1000));

    let mut receive_fork = BlockBuilder::receive()
        .previous(ctx.open_block.hash())
        .source(send.hash())
        .sign(ctx.receiver_key.clone())
        .without_sideband()
        .build()
        .unwrap();

    let result = ctx
        .ledger_context
        .process(ctx.txn.as_mut(), &mut receive_fork);

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn double_receive() {
    let mut ctx = LedgerWithOpenBlock::new();

    let mut double_receive = BlockBuilder::receive()
        .previous(ctx.open_block.hash())
        .source(ctx.send_block.hash())
        .sign(ctx.receiver_key)
        .build()
        .unwrap();

    let result = ctx
        .ledger_context
        .process(ctx.txn.as_mut(), &mut double_receive);

    assert_eq!(result, ProcessResult::Unreceivable);
}
