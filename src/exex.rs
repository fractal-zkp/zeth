use super::{db::Database, tracer::trace_block};
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::FullNodeComponents;
use reth_primitives::{Receipt, SealedBlockWithSenders};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tracing::info;

/// ZeroTracerExEx
#[derive(Debug)]
pub struct ZeroTracerExEx<Node: FullNodeComponents> {
    pub(crate) ctx: ExExContext<Node>,
    pub(crate) db: Database,
}

impl<Node: FullNodeComponents> ZeroTracerExEx<Node> {
    /// Construct a new ZeroTracerExEx instance.
    pub fn new(ctx: ExExContext<Node>, connection: Arc<Mutex<Connection>>) -> eyre::Result<Self> {
        Ok(Self { ctx, db: Database::new(connection)? })
    }

    /// Run the ZeroTracerExEx.
    pub async fn run(mut self) -> eyre::Result<()> {
        while let Some(notification) = self.ctx.notifications.recv().await {
            if let Some(reverted_chain) = notification.reverted_chain() {
                for (_, block) in reverted_chain.blocks() {
                    self.revert_block(block).await?;
                }
            }

            if let Some(committed_chain) = notification.committed_chain() {
                println!("{:?}", committed_chain.execution_outcome().state());
                for (block, receipts) in committed_chain.blocks_and_receipts() {
                    self.trace_and_commit_block(block.clone(), receipts.clone())?;
                }
                self.ctx.events.send(ExExEvent::FinishedHeight(committed_chain.tip().number))?;
            }
        }
        Ok(())
    }

    /// Process a new block commit.
    pub(crate) fn trace_and_commit_block(
        &mut self,
        block: SealedBlockWithSenders,
        receipts: Vec<Option<Receipt>>,
    ) -> eyre::Result<()> {
        let block_number = (&block).header().number;
        let block_hash = (&block).hash();
        info!("Processing block {} - {}", block_number, block_hash);
        let block_trace = trace_block(&mut self.ctx, block, receipts)?;
        self.db.commit_block_trace(block_hash, block_number, block_trace)?;
        Ok(())
    }

    /// Process a block revert.
    pub(crate) async fn revert_block(
        &mut self,
        block: &SealedBlockWithSenders,
    ) -> eyre::Result<()> {
        let block_hash = (&block).hash();
        info!("Reverting block {}", block_hash);
        self.db.delete_block_trace_by_hash(block_hash)?;
        Ok(())
    }
}
