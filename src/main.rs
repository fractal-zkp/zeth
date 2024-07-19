//! Polygon Zero exex

mod db;
mod error;
mod exex;
mod rpc;
mod tracer;

pub const DATABASE_PATH: &str = "polygon-zero.db";

fn main() {
    use exex::ZeroTracerExEx;
    use reth::cli::Cli;
    use reth_node_ethereum::EthereumNode;
    use rpc::{ZeroTracerRpc, ZeroTracerRpcApiServer};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = Cli::parse_args().run(|builder, _| async move {
        let connection = Arc::new(Mutex::new(Connection::open(DATABASE_PATH)?));
        let connection_rpc = connection.clone();
        let handle = builder
            .node(EthereumNode::default())
            .install_exex("ZeroTracerExEx", move |ctx| {
                let connection = connection.clone();
                async move {
                    let exex = ZeroTracerExEx::new(ctx, connection)?;
                    Ok(exex.run())
                }
            })
            .extend_rpc_modules(move |ctx| {
                let zero_rpc = ZeroTracerRpc::new(connection_rpc)?;
                ctx.modules.merge_configured(zero_rpc.into_rpc())?;
                Ok(())
            })
            .launch()
            .await?;

        handle.wait_for_node_exit().await
    }) {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
