use crate::{bitcoin::TxOut, BlockExtra};

mod mem;

#[cfg(feature = "db")]
mod db;

mod redb;

pub use mem::MemUtxo;
pub use redb::RedbUtxo;

use bitcoin::OutPoint;
#[cfg(feature = "db")]
pub use db::DbUtxo;

pub trait UtxoStore {
    /// Add all the outputs (except provably unspenof all the transaction in the block in the `UtxoStore`
    /// Return all the prevouts in the block at `height` in the order they are found in the block.
    /// First element in the vector is the prevout of the first input of the first transaction after
    /// the coinbase
    fn add_outputs_get_inputs(&mut self, block_extra: &BlockExtra, height: u32) -> Vec<TxOut>;

    /// return stats about the Utxo
    fn stat(&self) -> String;
}

trait Hash64 {
    fn hash64(&self) -> u64;
}

pub enum AnyUtxo {
    #[cfg(feature = "db")]
    Db(db::DbUtxo),
    Mem(MemUtxo),
    Redb(redb::RedbUtxo),
}

impl UtxoStore for AnyUtxo {
    fn add_outputs_get_inputs(&mut self, block_extra: &BlockExtra, height: u32) -> Vec<TxOut> {
        match self {
            #[cfg(feature = "db")]
            AnyUtxo::Db(db) => db.add_outputs_get_inputs(block_extra, height),
            AnyUtxo::Mem(mem) => mem.add_outputs_get_inputs(block_extra, height),
            AnyUtxo::Redb(db) => db.add_outputs_get_inputs(block_extra, height),
        }
    }

    fn stat(&self) -> String {
        match self {
            #[cfg(feature = "db")]
            AnyUtxo::Db(db) => db.stat(),
            AnyUtxo::Mem(mem) => mem.stat(),
            AnyUtxo::Redb(db) => db.stat(),
        }
    }
}

impl Hash64 for OutPoint {
    fn hash64(&self) -> u64 {
        fxhash::hash64(self)
    }
}
