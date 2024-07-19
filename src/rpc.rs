use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use reth_primitives::{BlockNumberOrTag, B256};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use trace_decoder::BlockTrace;

use super::{db::Database, error::ZeroApiError};

/// trait interface for zero tracer rpc.
///
/// This defines an additional zero namespace where all zero tracer methods are defined.
#[rpc(server, namespace = "zero")]
pub trait ZeroTracerRpcApi {
    /// Get block trace by block number.
    #[method(name = "getBlockTraceByNumber")]
    fn zero_trace_block_by_number(&self, number: BlockNumberOrTag) -> RpcResult<BlockTrace>;

    /// Get block trace by block hash.
    #[method(name = "getBlockTraceByHash")]
    fn zero_trace_block_by_hash(&self, hash: B256) -> RpcResult<BlockTrace>;
}

/// Zero Tracer RPC implementation.
#[derive(Debug)]
pub struct ZeroTracerRpc {
    /// Database instance.
    db: Database,
}

impl ZeroTracerRpc {
    /// Construct a new ZeroTracerRpc instance.
    pub fn new(connection: Arc<Mutex<Connection>>) -> Result<Self, ZeroApiError> {
        Ok(Self {
            db: Database::new(connection)?,
        })
    }
}

impl ZeroTracerRpcApiServer for ZeroTracerRpc {
    fn zero_trace_block_by_number(&self, number: BlockNumberOrTag) -> RpcResult<BlockTrace> {
        let number = if let BlockNumberOrTag::Number(number) = number {
            number
        } else {
            panic!("BlockNumberOrTag::Tag not supported")
        };

        let trace = self
            .db
            .get_block_trace_by_number(number)
            .map_err(ZeroApiError::DatabaseError)?;
        trace.ok_or(ZeroApiError::TraceNotFoundForBlockNumber(number).into())
    }

    fn zero_trace_block_by_hash(&self, hash: B256) -> RpcResult<BlockTrace> {
        let trace = self
            .db
            .get_block_trace_by_hash(hash)
            .map_err(ZeroApiError::DatabaseError)?;
        trace.ok_or(ZeroApiError::TraceNotFoundForBlockHash(hash.to_string()).into())
    }
}
