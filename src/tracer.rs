use alloy_rlp::{BufMut, Encodable};
use compat::Compat;
use eyre::Result;
use mpt_trie::builder::PartialTrieBuilder;
use reth_evm::{ConfigureEvm, ConfigureEvmEnv};
use reth_exex::ExExContext;
use reth_node_api::FullNodeComponents;
use reth_primitives::{
    keccak256, Address, Header, Receipt, SealedBlockWithSenders, StorageKey, TransactionSigned,
    B256,
};
use reth_provider::{EvmEnvProvider, StateProvider, StateProviderFactory};
use reth_revm::{database::StateProviderDatabase, primitives::state::EvmState};
use reth_trie::StorageWitness;
use revm::{
    db::CacheDB,
    primitives::{
        env::{BlockEnv, CfgEnvWithHandlerCfg, Env},
        EnvWithHandlerCfg, ResultAndState, TxEnv,
    },
    DatabaseCommit,
};
use std::collections::{HashMap, HashSet};
use trace_decoder::{
    BlockTrace, BlockTraceTriePreImages, ContractCodeUsage, SeparateStorageTriesPreImage,
    SeparateTriePreImage, SeparateTriePreImages, TxnInfo, TxnMeta, TxnTrace,
};

pub(crate) fn trace_block<Node: FullNodeComponents>(
    ctx: &mut ExExContext<Node>,
    block: SealedBlockWithSenders,
    receipts: Vec<Option<Receipt>>,
) -> Result<BlockTrace> {
    let beneficiary = block.beneficiary;
    let withdrawals: Vec<_> = block
        .block
        .withdrawals
        .clone()
        .map(|w| w.iter().map(|w| w.address).collect())
        .unwrap_or_default();
    let (cfg, block_env) = configure_evm(ctx, &block.header)?;
    let mut db = configure_db(ctx, &block);

    let mut transactions = block
        .into_transactions_ecrecovered()
        .zip(receipts.into_iter())
        .peekable();

    // instantiate state access
    let mut state_access: HashMap<Address, HashSet<StorageKey>> = HashMap::new();

    // populate beneficiary and withdrawals
    state_access.insert(beneficiary, HashSet::new());
    for address in withdrawals {
        state_access.insert(address, HashSet::new());
    }

    let mut code_db = HashMap::new();
    let mut txn_infos = vec![];
    let mut cum_gas = 0;

    while let Some((tx, receipt)) = transactions.next() {
        let tx_env = ctx.evm_config().tx_env(&tx);
        let receipt = receipt.expect("receipt should be present");
        let env = create_tx_env(tx_env, &cfg, &block_env);
        let mut evm = ctx.evm_config().evm_with_env(&mut db, env);
        let ResultAndState { state, result: _ } = evm.transact()?;
        txn_infos.push(trace_transaction(
            &tx,
            receipt,
            &state,
            &mut code_db,
            &mut cum_gas,
            &mut state_access,
        ));

        std::mem::drop(evm);

        if transactions.peek().is_some() {
            db.commit(state);
        }
    }

    let trie_pre_images = state_witness(db.db.0, state_access)?;

    Ok(BlockTrace {
        trie_pre_images,
        code_db: Some(code_db),
        txn_info: txn_infos,
    })
}

fn configure_db<Node: FullNodeComponents>(
    ctx: &mut ExExContext<Node>,
    block: &SealedBlockWithSenders,
) -> CacheDB<StateProviderDatabase<Box<dyn StateProvider>>> {
    let block_hash = block.parent_hash;
    let state = ctx.provider().state_by_block_hash(block_hash).unwrap();
    CacheDB::new(StateProviderDatabase::new(state))
}

fn configure_evm<Node: FullNodeComponents>(
    ctx: &mut ExExContext<Node>,
    header: &Header,
) -> Result<(CfgEnvWithHandlerCfg, BlockEnv)> {
    let mut cfg = CfgEnvWithHandlerCfg::new(Default::default(), Default::default());
    let mut block_env = BlockEnv::default();
    ctx.provider().fill_env_with_header::<Node::Evm>(
        &mut cfg,
        &mut block_env,
        header,
        ctx.evm_config().clone(),
    )?;

    Ok((cfg, block_env))
}

fn create_tx_env(
    tx_env: TxEnv,
    cfg: &CfgEnvWithHandlerCfg,
    block_env: &BlockEnv,
) -> EnvWithHandlerCfg {
    EnvWithHandlerCfg {
        env: Env::boxed(cfg.cfg_env.clone(), block_env.clone(), tx_env),
        handler_cfg: cfg.handler_cfg,
    }
}

fn trace_transaction(
    tx: &TransactionSigned,
    receipt: Receipt,
    state: &EvmState,
    code_db: &mut HashMap<primitive_types::H256, Vec<u8>>,
    cum_gas: &mut u64,
    state_access: &mut HashMap<Address, HashSet<StorageKey>>,
) -> TxnInfo {
    let meta = TxnMeta {
        byte_code: tx.envelope_encoded().to_vec(),
        new_txn_trie_node_byte: tx.envelope_encoded().to_vec(),
        gas_used: {
            let previous_cum_gas = std::mem::replace(cum_gas, receipt.cumulative_gas_used);
            receipt.cumulative_gas_used - previous_cum_gas
        },
        new_receipt_trie_node_byte: {
            let mut buf = vec![];
            receipt.with_bloom().encode(&mut buf as &mut dyn BufMut);
            buf
        },
    };

    let traces = state
        .into_iter()
        .map(|(address, state)| {
            let account_state = state_access.entry(*address).or_default();
            let mut storage_read = vec![];
            let mut storage_written: HashMap<primitive_types::H256, _> = HashMap::new();

            for (key, value) in state.storage.clone().into_iter() {
                match value.is_changed() {
                    true => {
                        storage_written.insert(
                            Into::<B256>::into(key).compat(),
                            value.present_value.compat(),
                        );
                        account_state.insert(key.into());
                    }
                    false => {
                        storage_read.push(Into::<B256>::into(key).compat());
                        account_state.insert(key.into());
                    }
                }
            }

            let code_usage =
                match state.info.is_empty_code_hash() || state.info.code_hash() == B256::ZERO {
                    true => None,
                    false => state.info.code.clone().map(|code| {
                        code_db.insert(
                            state.info.code_hash.compat(),
                            code.original_bytes().to_vec(),
                        );
                        match state.is_created() {
                            true => ContractCodeUsage::Write(code.original_bytes().to_vec().into()),
                            false => ContractCodeUsage::Read(state.info.code_hash.compat()),
                        }
                    }),
                };

            let trace = TxnTrace {
                balance: Some(state.info.balance.compat()).filter(|_| state.is_touched()),
                nonce: Some(state.info.nonce.into()).filter(|_| state.is_touched()),
                storage_read: Some(storage_read).filter(|x| !x.is_empty()),
                storage_written: Some(storage_written).filter(|x| !x.is_empty()),
                code_usage,
                self_destructed: Some(state.is_selfdestructed()).filter(|x| *x),
            };

            ((*address).compat(), trace)
        })
        .collect();

    TxnInfo { meta, traces }
}

fn state_witness(
    state: Box<dyn StateProvider>,
    state_access: HashMap<Address, HashSet<StorageKey>>,
) -> Result<BlockTraceTriePreImages> {
    // fetch the state witness
    let state_access: Vec<(Address, Vec<StorageKey>)> = state_access
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect();
    let state_witness = state.witness(&Default::default(), state_access)?;

    // build the account trie witness
    let mut state_trie_builder =
        PartialTrieBuilder::new(state_witness.state_root.compat(), Default::default());
    state_trie_builder.insert_proof(state_witness.accounts_witness.compat());

    // build the storage trie witnesses
    let storage_witnesses = state_witness
        .storage_witnesses
        .into_iter()
        .map(
            |StorageWitness {
                 address,
                 storage_root,
                 storage_witness,
             }| {
                let mut storage_trie_builder =
                    PartialTrieBuilder::new(storage_root.compat(), Default::default());
                storage_trie_builder.insert_proof(storage_witness.compat());
                (
                    keccak256(address).compat(),
                    SeparateTriePreImage::Direct(storage_trie_builder.build()),
                )
            },
        )
        .collect();

    Ok(BlockTraceTriePreImages::Separate(SeparateTriePreImages {
        state: SeparateTriePreImage::Direct(state_trie_builder.build()),
        storage: SeparateStorageTriesPreImage::MultipleTries(storage_witnesses),
    }))
}
