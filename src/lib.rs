//! # Blocks Iterator
//!
//! Read bitcoin blocks directory containing `blocks*.dat` files, and produce a ordered stream
//! of [BlockExtra]
//!
use bitcoin::{Block, BlockHash, OutPoint, Transaction, TxOut, Txid};
use log::{info, Level};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;
use structopt::StructOpt;

mod fee;
mod parse;
mod read;
mod reorder;
mod truncmap;

/// Configuration parameters, most important the bitcoin blocks directory
#[derive(StructOpt, Debug, Clone)]
pub struct Config {
    /// Blocks directory (containing `blocks*.dat`)
    #[structopt(short, long)]
    pub blocks_dir: PathBuf,

    /// Network (bitcoin, testnet, regtest, signet)
    #[structopt(short, long)]
    pub network: bitcoin::Network,

    /// Skip calculation of previous outputs, it's faster and it uses much less memory
    /// however make it impossible calculate fees or access tx input previous scripts
    #[structopt(short, long)]
    pub skip_prevout: bool,

    /// Maximum length of a reorg allowed, during reordering send block to the next step only
    /// if it has `max_reorg` following blocks. Higher is more conservative, while lower faster.
    /// When parsing testnet blocks, it may be necessary to increase this a lot
    #[structopt(short, long, default_value = "6")]
    pub max_reorg: u8,
}

/// The bitcoin block and additional metadata returned by the [iterate] method
#[derive(Debug)]
pub struct BlockExtra {
    /// The bitcoin block
    pub block: Block,
    /// The bitcoin block hash, same as `block.block_hash()` but result from hashing is cached
    pub block_hash: BlockHash,
    /// The byte size of the block, as returned by in `serialize(block).len()`
    pub size: u32,
    /// Hash of the blocks following this one, it's a vec because during reordering they may be more
    /// than one because of reorgs, as a result from [iterate], it's just one.
    pub next: Vec<BlockHash>,
    /// The height of the current block, number of blocks between this one and the genesis block
    pub height: u32,
    /// All the previous outputs of this block. Allowing to validate the script or computing the fee
    /// Note that when configuration `skip_script_pubkey` is true, the script is empty,
    /// when `skip_prevout` is true, this map is empty.
    pub outpoint_values: HashMap<OutPoint, TxOut>,
    /// Precomputed set of txid present in `block`
    pub tx_hashes: HashSet<Txid>,
}

impl BlockExtra {
    /// Returns the average transaction fee in the block
    pub fn average_fee(&self) -> Option<f64> {
        Some(self.fee()? as f64 / self.block.txdata.len() as f64)
    }

    /// Returns the total fee of the block
    pub fn fee(&self) -> Option<u64> {
        let mut total = 0u64;
        for tx in self.block.txdata.iter() {
            total += self.tx_fee(tx)?;
        }
        Some(total)
    }

    /// Returns the fee of a transaction contained in the block
    pub fn tx_fee(&self, tx: &Transaction) -> Option<u64> {
        let output_total: u64 = tx.output.iter().map(|el| el.value).sum();
        let mut input_total = 0u64;
        for input in tx.input.iter() {
            input_total += self.outpoint_values.get(&input.previous_output)?.value;
        }
        Some(input_total - output_total)
    }
}

/// Read `blocks*.dat` contained in the `config.blocks_dir` directory and returns [BlockExtra]
/// through a channel supplied from the caller. Blocks returned are ordered from the genesis to the
/// highest block in the dircetory (minus `config.max_reorg`).
/// In this call threads are spawned, caller must call [std::thread::JoinHandle::join] on the returning handle.
pub fn iterate(config: Config, channel: SyncSender<Option<BlockExtra>>) -> JoinHandle<()> {
    thread::spawn(move || {
        let now = Instant::now();

        let (send_blobs, receive_blobs) = sync_channel(2);

        let mut read = read::Read::new(config.blocks_dir.clone(), send_blobs);
        let read_handle = thread::spawn(move || {
            read.start();
        });

        let (send_blocks, receive_blocks) = sync_channel(200);
        let mut parse = parse::Parse::new(config.network, receive_blobs, send_blocks);
        let parse_handle = thread::spawn(move || {
            parse.start();
        });

        let (send_ordered_blocks, receive_ordered_blocks) = sync_channel(200);
        let send_ordered_blocks = if config.skip_prevout {
            // if skip_prevout is true, we send directly to end step
            channel.clone()
        } else {
            send_ordered_blocks
        };
        let mut reorder = reorder::Reorder::new(
            config.network,
            config.max_reorg,
            receive_blocks,
            send_ordered_blocks,
        );
        let orderer_handle = thread::spawn(move || {
            reorder.start();
        });

        if !config.skip_prevout {
            let mut fee = fee::Fee::new(receive_ordered_blocks, channel);
            let fee_handle = thread::spawn(move || {
                fee.start();
            });
            fee_handle.join().unwrap();
        }

        read_handle.join().unwrap();
        parse_handle.join().unwrap();
        orderer_handle.join().unwrap();
        info!("Total time elapsed: {}s", now.elapsed().as_secs());
    })
}

/// Utility method usually returning [log::Level::Debug] but when `i` is divisible by `10_000` returns [log::Level::Info]
pub fn periodic_log_level(i: u32) -> Level {
    if i % 10_000 == 0 {
        Level::Info
    } else {
        Level::Debug
    }
}
