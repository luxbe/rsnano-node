mod block_details;
pub use block_details::BlockDetails;

mod block_sideband;
pub use block_sideband::BlockSideband;

mod change_block;
pub use change_block::{valid_change_block_predecessor, ChangeBlock, ChangeHashables};

mod open_block;
use once_cell::sync::Lazy;
pub use open_block::{OpenBlock, OpenHashables};

mod receive_block;
pub use receive_block::{valid_receive_block_predecessor, ReceiveBlock, ReceiveHashables};

mod send_block;
pub use send_block::{valid_send_block_predecessor, SendBlock, SendHashables};

mod state_block;
pub use state_block::{StateBlock, StateHashables};

mod builders;
pub use builders::*;

use crate::{
    utils::{
        Deserialize, MemoryStream, PropertyTreeReader, PropertyTreeWriter, SerdePropertyTree,
        Stream, StreamAdapter,
    },
    Account, Amount, BlockHash, BlockHashBuilder, Epoch, FullHash, KeyPair, Link, QualifiedRoot,
    Root, Signature, WorkVersion,
};
use num::FromPrimitive;
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive)]
pub enum BlockType {
    Invalid = 0,
    NotABlock = 1,
    LegacySend = 2,
    LegacyReceive = 3,
    LegacyOpen = 4,
    LegacyChange = 5,
    State = 6,
}

impl TryFrom<u8> for BlockType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        FromPrimitive::from_u8(value).ok_or_else(|| anyhow!("invalid block type value"))
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum BlockSubType {
    Send,
    Receive,
    Open,
    Change,
    Epoch,
}

#[derive(Clone, Default, Debug)]
pub struct LazyBlockHash {
    // todo: Remove Arc<RwLock>? Maybe remove lazy hash calculation?
    hash: Arc<RwLock<BlockHash>>,
}

impl LazyBlockHash {
    pub fn new() -> Self {
        Self {
            hash: Arc::new(RwLock::new(BlockHash::zero())),
        }
    }
    pub fn hash(&'_ self, factory: impl Into<BlockHash>) -> BlockHash {
        let mut value = self.hash.read().unwrap();
        if value.is_zero() {
            drop(value);
            let mut x = self.hash.write().unwrap();
            let block_hash: BlockHash = factory.into();
            *x = block_hash;
            drop(x);
            value = self.hash.read().unwrap();
        }

        *value
    }

    pub fn clear(&self) {
        let mut x = self.hash.write().unwrap();
        *x = BlockHash::zero();
    }
}

pub trait Block: FullHash {
    fn block_type(&self) -> BlockType;
    fn account(&self) -> Account;

    /**
     * Contextual details about a block, some fields may or may not be set depending on block type.
     * This field is set via sideband_set in ledger processing or deserializing blocks from the database.
     * Otherwise it may be null (for example, an old block or fork).
     */
    fn sideband(&'_ self) -> Option<&'_ BlockSideband>;
    fn set_sideband(&mut self, sideband: BlockSideband);
    fn hash(&self) -> BlockHash;
    fn link(&self) -> Link;
    fn block_signature(&self) -> &Signature;
    fn set_block_signature(&mut self, signature: &Signature);
    fn work(&self) -> u64;
    fn set_work(&mut self, work: u64);
    fn previous(&self) -> BlockHash;
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()>;
    fn serialize_json(&self, writer: &mut dyn PropertyTreeWriter) -> anyhow::Result<()>;
    fn to_json(&self) -> anyhow::Result<String> {
        let mut writer = SerdePropertyTree::new();
        self.serialize_json(&mut writer)?;
        Ok(writer.to_json())
    }
    fn work_version(&self) -> WorkVersion {
        WorkVersion::Work1
    }
    fn root(&self) -> Root;
    fn visit(&self, visitor: &mut dyn BlockVisitor);
    fn visit_mut(&mut self, visitor: &mut dyn MutableBlockVisitor);
    fn balance(&self) -> Amount;
    fn source(&self) -> Option<BlockHash>;
    fn representative(&self) -> Option<Account>;
    fn destination(&self) -> Option<Account>;
    fn qualified_root(&self) -> QualifiedRoot {
        QualifiedRoot::new(self.root(), self.previous())
    }
    fn valid_predecessor(&self, block_type: BlockType) -> bool;
}

impl<T: Block> FullHash for T {
    fn full_hash(&self) -> BlockHash {
        BlockHashBuilder::new()
            .update(self.hash().as_bytes())
            .update(self.block_signature().as_bytes())
            .update(self.work().to_ne_bytes())
            .build()
    }
}

pub trait BlockVisitor {
    fn send_block(&mut self, block: &SendBlock);
    fn receive_block(&mut self, block: &ReceiveBlock);
    fn open_block(&mut self, block: &OpenBlock);
    fn change_block(&mut self, block: &ChangeBlock);
    fn state_block(&mut self, block: &StateBlock);
}

pub trait MutableBlockVisitor {
    fn send_block(&mut self, block: &mut SendBlock);
    fn receive_block(&mut self, block: &mut ReceiveBlock);
    fn open_block(&mut self, block: &mut OpenBlock);
    fn change_block(&mut self, block: &mut ChangeBlock);
    fn state_block(&mut self, block: &mut StateBlock);
}

pub fn serialized_block_size(block_type: BlockType) -> usize {
    match block_type {
        BlockType::Invalid | BlockType::NotABlock => 0,
        BlockType::LegacySend => SendBlock::serialized_size(),
        BlockType::LegacyReceive => ReceiveBlock::serialized_size(),
        BlockType::LegacyOpen => OpenBlock::serialized_size(),
        BlockType::LegacyChange => ChangeBlock::serialized_size(),
        BlockType::State => StateBlock::serialized_size(),
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BlockEnum {
    LegacySend(SendBlock),
    LegacyReceive(ReceiveBlock),
    LegacyOpen(OpenBlock),
    LegacyChange(ChangeBlock),
    State(StateBlock),
}

impl BlockEnum {
    pub fn block_type(&self) -> BlockType {
        self.as_block().block_type()
    }

    pub fn as_block_mut(&mut self) -> &mut dyn Block {
        match self {
            BlockEnum::LegacySend(b) => b,
            BlockEnum::LegacyReceive(b) => b,
            BlockEnum::LegacyOpen(b) => b,
            BlockEnum::LegacyChange(b) => b,
            BlockEnum::State(b) => b,
        }
    }

    pub fn as_block(&self) -> &dyn Block {
        match self {
            BlockEnum::LegacySend(b) => b,
            BlockEnum::LegacyReceive(b) => b,
            BlockEnum::LegacyOpen(b) => b,
            BlockEnum::LegacyChange(b) => b,
            BlockEnum::State(b) => b,
        }
    }

    pub fn balance_calculated(&self) -> Amount {
        match self {
            BlockEnum::LegacySend(b) => b.balance(),
            BlockEnum::LegacyReceive(b) => b.sideband().unwrap().balance,
            BlockEnum::LegacyOpen(b) => b.sideband().unwrap().balance,
            BlockEnum::LegacyChange(b) => b.sideband().unwrap().balance,
            BlockEnum::State(b) => b.balance(),
        }
    }

    pub fn is_open(&self) -> bool {
        match &self {
            BlockEnum::LegacyOpen(_) => true,
            BlockEnum::State(state) => state.previous().is_zero(),
            _ => false,
        }
    }

    pub fn is_legacy(&self) -> bool {
        !matches!(self, BlockEnum::State(_))
    }

    pub fn balance_opt(&self) -> Option<Amount> {
        match self {
            BlockEnum::LegacySend(b) => Some(b.balance()),
            BlockEnum::State(b) => Some(b.balance()),
            _ => None,
        }
    }

    pub fn source_or_link(&self) -> BlockHash {
        self.source().unwrap_or_else(|| self.link().into())
    }

    pub fn destination_or_link(&self) -> Account {
        self.destination().unwrap_or_else(|| self.link().into())
    }

    pub fn account_calculated(&self) -> Account {
        let result = if self.account().is_zero() {
            self.sideband().unwrap().account
        } else {
            self.account()
        };

        result
    }

    pub fn height(&self) -> u64 {
        self.sideband().map(|s| s.height).unwrap_or_default()
    }

    pub fn successor(&self) -> Option<BlockHash> {
        if let Some(sideband) = self.sideband() {
            if !sideband.successor.is_zero() {
                Some(sideband.successor)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn epoch(&self) -> Epoch {
        self.sideband().unwrap().details.epoch
    }

    pub fn serialize_with_sideband(&self) -> Vec<u8> {
        let mut stream = MemoryStream::new();
        stream.write_u8(self.block_type() as u8).unwrap();
        self.serialize(&mut stream).unwrap();
        self.sideband()
            .unwrap()
            .serialize(&mut stream, self.block_type())
            .unwrap();
        stream.to_vec()
    }

    pub fn deserialize_with_sideband(bytes: &[u8]) -> anyhow::Result<BlockEnum> {
        let mut stream = StreamAdapter::new(bytes);
        let mut block = deserialize_block_enum(&mut stream)?;
        let mut sideband = BlockSideband::from_stream(&mut stream, block.block_type())?;
        // BlockSideband does not serialize all data depending on the block type.
        // That's why we fill in the missing data here:
        match &block {
            BlockEnum::LegacySend(_) => {
                sideband.balance = block.balance();
                sideband.details = BlockDetails::new(Epoch::Epoch0, true, false, false)
            }
            BlockEnum::LegacyOpen(_) => {
                sideband.account = block.account();
                sideband.details = BlockDetails::new(Epoch::Epoch0, false, true, false)
            }
            BlockEnum::LegacyReceive(_) => {
                sideband.details = BlockDetails::new(Epoch::Epoch0, false, true, false)
            }
            BlockEnum::LegacyChange(_) => {
                sideband.details = BlockDetails::new(Epoch::Epoch0, false, false, false)
            }
            BlockEnum::State(_) => {
                sideband.account = block.account();
                sideband.balance = block.balance();
            }
        }
        block.as_block_mut().set_sideband(sideband);
        Ok(block)
    }
}

impl FullHash for BlockEnum {
    fn full_hash(&self) -> BlockHash {
        self.as_block().full_hash()
    }
}

impl Deref for BlockEnum {
    type Target = dyn Block;

    fn deref(&self) -> &Self::Target {
        match self {
            BlockEnum::LegacySend(b) => b,
            BlockEnum::LegacyReceive(b) => b,
            BlockEnum::LegacyOpen(b) => b,
            BlockEnum::LegacyChange(b) => b,
            BlockEnum::State(b) => b,
        }
    }
}

impl DerefMut for BlockEnum {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            BlockEnum::LegacySend(b) => b,
            BlockEnum::LegacyReceive(b) => b,
            BlockEnum::LegacyOpen(b) => b,
            BlockEnum::LegacyChange(b) => b,
            BlockEnum::State(b) => b,
        }
    }
}

pub fn deserialize_block_json(ptree: &impl PropertyTreeReader) -> anyhow::Result<BlockEnum> {
    let block_type = ptree.get_string("type")?;
    match block_type.as_str() {
        "receive" => ReceiveBlock::deserialize_json(ptree).map(BlockEnum::LegacyReceive),
        "send" => SendBlock::deserialize_json(ptree).map(BlockEnum::LegacySend),
        "open" => OpenBlock::deserialize_json(ptree).map(BlockEnum::LegacyOpen),
        "change" => ChangeBlock::deserialize_json(ptree).map(BlockEnum::LegacyChange),
        "state" => StateBlock::deserialize_json(ptree).map(BlockEnum::State),
        _ => Err(anyhow!("unsupported block type")),
    }
}

pub fn serialize_block_enum(stream: &mut dyn Stream, block: &BlockEnum) -> anyhow::Result<()> {
    let block_type = block.block_type() as u8;
    stream.write_u8(block_type)?;
    block.serialize(stream)
}

pub fn deserialize_block_enum(stream: &mut dyn Stream) -> anyhow::Result<BlockEnum> {
    let block_type =
        BlockType::from_u8(stream.read_u8()?).ok_or_else(|| anyhow!("invalid block type"))?;
    deserialize_block_enum_with_type(block_type, stream)
}

pub fn deserialize_block_enum_with_type(
    block_type: BlockType,
    stream: &mut dyn Stream,
) -> anyhow::Result<BlockEnum> {
    let block = match block_type {
        BlockType::LegacyReceive => BlockEnum::LegacyReceive(ReceiveBlock::deserialize(stream)?),
        BlockType::LegacyOpen => BlockEnum::LegacyOpen(OpenBlock::deserialize(stream)?),
        BlockType::LegacyChange => BlockEnum::LegacyChange(ChangeBlock::deserialize(stream)?),
        BlockType::State => BlockEnum::State(StateBlock::deserialize(stream)?),
        BlockType::LegacySend => BlockEnum::LegacySend(SendBlock::deserialize(stream)?),
        BlockType::Invalid | BlockType::NotABlock => bail!("invalid block type"),
    };
    Ok(block)
}

pub struct BlockWithSideband {
    pub block: BlockEnum,
    pub sideband: BlockSideband,
}

impl Deserialize for BlockWithSideband {
    type Target = Self;
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self> {
        let mut block = deserialize_block_enum(stream)?;
        let sideband = BlockSideband::from_stream(stream, block.block_type())?;
        block.as_block_mut().set_sideband(sideband.clone());
        Ok(BlockWithSideband { block, sideband })
    }
}

pub fn serialize_block<T: Stream>(stream: &mut T, block: &BlockEnum) -> anyhow::Result<()> {
    stream.write_u8(block.block_type() as u8)?;
    block.serialize(stream)
}

static DEV_PRIVATE_KEY_DATA: &str =
    "34F0A37AAD20F4A260F0A5B3CB3D7FB50673212263E58A380BC10474BB039CE4";
pub static DEV_PUBLIC_KEY_DATA: &str =
    "B0311EA55708D6A53C75CDBF88300259C6D018522FE3D4D0A242E431F9E8B6D0"; // xrb_3e3j5tkog48pnny9dmfzj1r16pg8t1e76dz5tmac6iq689wyjfpiij4txtdo
pub static DEV_GENESIS_KEY: Lazy<KeyPair> =
    Lazy::new(|| KeyPair::from_priv_key_hex(DEV_PRIVATE_KEY_DATA).unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_legacy_open() {
        let block = BlockBuilder::legacy_open().with_sideband().build();
        assert_serializable(block);
    }

    #[test]
    fn serialize_legacy_receive() {
        let block = BlockBuilder::legacy_receive().with_sideband().build();
        assert_serializable(block);
    }

    #[test]
    fn serialize_legacy_send() {
        let block = BlockBuilder::legacy_send().with_sideband().build();
        assert_serializable(block);
    }

    #[test]
    fn serialize_legacy_change() {
        let block = BlockBuilder::legacy_change().with_sideband().build();
        assert_serializable(block);
    }

    #[test]
    fn serialize_state() {
        let block = BlockBuilder::state().with_sideband().build();
        assert_serializable(block);
    }

    fn assert_serializable(block: BlockEnum) {
        let bytes = block.serialize_with_sideband();
        let deserialized = BlockEnum::deserialize_with_sideband(&bytes).unwrap();

        assert_eq!(deserialized, block);
    }
}
