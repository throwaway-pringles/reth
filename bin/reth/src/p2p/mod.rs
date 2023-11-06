//! P2P Debugging tool
use crate::{
    args::{
        get_secret_key,
        utils::{chain_spec_value_parser, hash_or_num_value_parser},
        DatabaseArgs, DiscoveryArgs,
    },
    dirs::{DataDirPath, MaybePlatformPath},
    utils::get_single_header,
};
use backon::{ConstantBuilder, Retryable};
use clap::{Parser, Subcommand};
use reth_config::Config;
use reth_db::open_db;
use reth_discv4::NatResolver;
use reth_interfaces::p2p::bodies::client::BodiesClient;
use reth_primitives::{BlockHashOrNumber, ChainSpec, NodeRecord};
use reth_provider::ProviderFactory;
use std::{path::PathBuf, sync::Arc};

/// `reth p2p` command
#[derive(Debug, Parser)]
pub struct Command {
    /// The path to the configuration file to use.
    #[arg(long, value_name = "FILE", verbatim_doc_comment)]
    config: Option<PathBuf>,

    /// The chain this node is running.
    ///
    /// Possible values are either a built-in chain or the path to a chain specification file.
    ///
    /// Built-in chains:
    /// - mainnet
    /// - goerli
    /// - sepolia
    /// - holesky
    #[arg(
        long,
        value_name = "CHAIN_OR_PATH",
        verbatim_doc_comment,
        default_value = "mainnet",
        value_parser = chain_spec_value_parser
    )]
    chain: Arc<ChainSpec>,

    /// The path to the data dir for all reth files and subdirectories.
    ///
    /// Defaults to the OS-specific data directory:
    ///
    /// - Linux: `$XDG_DATA_HOME/reth/` or `$HOME/.local/share/reth/`
    /// - Windows: `{FOLDERID_RoamingAppData}/reth/`
    /// - macOS: `$HOME/Library/Application Support/reth/`
    #[arg(long, value_name = "DATA_DIR", verbatim_doc_comment, default_value_t)]
    datadir: MaybePlatformPath<DataDirPath>,

    /// Secret key to use for this node.
    ///
    /// This also will deterministically set the peer ID.
    #[arg(long, value_name = "PATH")]
    p2p_secret_key: Option<PathBuf>,

    /// Disable the discovery service.
    #[command(flatten)]
    pub discovery: DiscoveryArgs,

    /// Target trusted peer
    #[arg(long)]
    trusted_peer: Option<NodeRecord>,

    /// Connect only to trusted peers
    #[arg(long)]
    trusted_only: bool,

    /// The number of retries per request
    #[arg(long, default_value = "5")]
    retries: usize,

    #[arg(long, default_value = "any")]
    nat: NatResolver,

    #[clap(flatten)]
    db: DatabaseArgs,

    #[clap(subcommand)]
    command: Subcommands,
}

/// `reth p2p` subcommands
#[derive(Subcommand, Debug)]
pub enum Subcommands {
    /// Download block header
    Header {
        /// The header number or hash
        #[arg(value_parser = hash_or_num_value_parser)]
        id: BlockHashOrNumber,
    },
    /// Download block body
    Body {
        /// The block number or hash
        #[arg(value_parser = hash_or_num_value_parser)]
        id: BlockHashOrNumber,
    },
}
impl Command {
    /// Execute `p2p` command
    pub async fn execute(&self) -> eyre::Result<()> {
        let tempdir = tempfile::TempDir::new()?;
        let noop_db = Arc::new(open_db(&tempdir.into_path(), self.db.log_level)?);

        // add network name to data dir
        let data_dir = self.datadir.unwrap_or_chain_default(self.chain.chain);
        let config_path = self.config.clone().unwrap_or(data_dir.config_path());

        let mut config: Config = confy::load_path(&config_path).unwrap_or_default();

        if let Some(peer) = self.trusted_peer {
            config.peers.trusted_nodes.insert(peer);
        }

        if config.peers.trusted_nodes.is_empty() && self.trusted_only {
            eyre::bail!("No trusted nodes. Set trusted peer with `--trusted-peer <enode record>` or set `--trusted-only` to `false`")
        }

        config.peers.connect_trusted_nodes_only = self.trusted_only;

        let default_secret_key_path = data_dir.p2p_secret_path();
        let secret_key_path = self.p2p_secret_key.clone().unwrap_or(default_secret_key_path);
        let p2p_secret_key = get_secret_key(&secret_key_path)?;

        let mut network_config_builder =
            config.network_config(self.nat, None, p2p_secret_key).chain_spec(self.chain.clone());

        network_config_builder = self.discovery.apply_to_builder(network_config_builder);

        let network = network_config_builder
            .build(Arc::new(ProviderFactory::new(noop_db, self.chain.clone())))
            .start_network()
            .await?;

        let fetch_client = network.fetch_client().await?;
        let retries = self.retries.max(1);
        let backoff = ConstantBuilder::default().with_max_times(retries);

        match self.command {
            Subcommands::Header { id } => {
                let header = (move || get_single_header(fetch_client.clone(), id))
                    .retry(&backoff)
                    .notify(|err, _| println!("Error requesting header: {err}. Retrying..."))
                    .await?;
                println!("Successfully downloaded header: {header:?}");
            }
            Subcommands::Body { id } => {
                let hash = match id {
                    BlockHashOrNumber::Hash(hash) => hash,
                    BlockHashOrNumber::Number(number) => {
                        println!("Block number provided. Downloading header first...");
                        let client = fetch_client.clone();
                        let header = (move || {
                            get_single_header(client.clone(), BlockHashOrNumber::Number(number))
                        })
                        .retry(&backoff)
                        .notify(|err, _| println!("Error requesting header: {err}. Retrying..."))
                        .await?;
                        header.hash()
                    }
                };
                let (_, result) = (move || {
                    let client = fetch_client.clone();
                    client.get_block_bodies(vec![hash])
                })
                .retry(&backoff)
                .notify(|err, _| println!("Error requesting block: {err}. Retrying..."))
                .await?
                .split();
                if result.len() != 1 {
                    eyre::bail!(
                        "Invalid number of headers received. Expected: 1. Received: {}",
                        result.len()
                    )
                }
                let body = result.into_iter().next().unwrap();
                println!("Successfully downloaded body: {body:?}")
            }
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::{sync::Arc, error::Error, path::PathBuf, net::Ipv4Addr, task::Poll};

    use futures::{future::poll_fn, FutureExt, StreamExt};
    use reth_config::Config;
    use reth_db::{open_db};
    use reth_discv4::{DEFAULT_DISCOVERY_PORT, DEFAULT_DISCOVERY_ADDR};
    use reth_interfaces::{db::LogLevel, p2p::error::RequestResult};
    use reth_network::{NetworkManager, transactions::NetworkTransactionEvent};
    use reth_primitives::{ChainSpec, ChainSpecBuilder};
    use reth_rpc_types::NodeRecord;
    use reth_transaction_pool::test_utils::testing_pool;
    use tokio::sync::oneshot;
    use crate::dirs::{MaybePlatformPath, PlatformPath, DataDirPath};
    use super::*;

    #[tokio::test]
    async fn get_header() {
        let tempdir = tempfile::TempDir::new().unwrap();
        let noop_db = Arc::new(open_db(&tempdir.into_path(), Some(LogLevel::Trace)).unwrap());

        let chain = Arc::new(ChainSpecBuilder::mainnet().build());
        
        // add network name to data dir
        let data_dir = MaybePlatformPath::<DataDirPath>::default().unwrap_or_chain_default(chain.chain);
        let config_path = data_dir.config_path();
        let mut config: Config = confy::load_path(&config_path).unwrap_or_default();


        // Only use discovery
        // let url = "enode://f22c7ba88f00fa95ba9f82caee230a53f49086449b54880ea69a4ba2faba83de6b68f9a47cd21002f557c89658ea8e88346b8ee4137fdd1cd25b6ac3ef2ea56d@127.0.0.1:30303?discport=30301";
        // let node: NodeRecord = url.parse().unwrap();
        // // if let Some(peer) = self.trusted_peer {
        //     config.peers.trusted_nodes.insert(node);
        // }
        
        let trusted_only = false;
        if config.peers.trusted_nodes.is_empty() && trusted_only {
            println!("No trusted nodes. Set trusted peer with `--trusted-peer <enode record>` or set `--trusted-only` to `false`");
        }

        config.peers.connect_trusted_nodes_only = trusted_only;

        let default_secret_key_path = data_dir.p2p_secret_path();
        let p2p_secret_key = get_secret_key(&default_secret_key_path).unwrap();

        let mut network_config_builder =
            config.network_config(NatResolver::Any, None, p2p_secret_key).chain_spec(chain.clone());

        let discovery: DiscoveryArgs = DiscoveryArgs {
            disable_discovery: false,
            disable_discv4_discovery: false,
            disable_dns_discovery: false,
            addr: DEFAULT_DISCOVERY_ADDR,
            port: DEFAULT_DISCOVERY_PORT,
        };
        network_config_builder = discovery.apply_to_builder(network_config_builder);

        let pool = testing_pool();
        let config = network_config_builder
            .build(Arc::new(ProviderFactory::new(noop_db, chain.clone())));
        
        let (network_handle, network, mut transactions, _) = NetworkManager::new(config)
            .await
            .unwrap()
            .into_builder()
            .transactions(pool.clone())
            .split_with_handle();
        
        let fetch_client = network.fetch_client();

        let mut events = network_handle.event_listener();

        let handle = network_handle.clone();

        tokio::task::spawn(async move {
            tokio::join!(network, transactions);
        });

        // Report currently connected peers every 5 sec
        // tokio::task::spawn(async move {
        //     loop {
        //         tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        //         // dbg!(handle.num_connected_peers());
        //     }
        // });

        println!("Network Started");

        let retries = 5;
        let backoff = ConstantBuilder::default().with_max_times(retries);
        let header = (move || get_single_header(fetch_client.clone(), BlockHashOrNumber::Number(18476854))).retry(&backoff)
            .notify(|err, _| println!("Error requesting header: {err}. Retrying..."))
            .await.unwrap();
        println!("Successfully downloaded header: {header:?}");

        // Attempt to get network events
        while let Some(ev) = events.next().await {
            dbg!(ev);
        }
        
        // Attempt to poll transactions
        // poll_fn(|cx| {
        //     let _ = transactions.poll_unpin(cx);
        //     Poll::Ready(())
        // })
        // .await;
        // assert!(!pool.is_empty());
    }
}