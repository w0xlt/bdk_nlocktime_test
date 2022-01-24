use bdk::bitcoin::secp256k1::Secp256k1;
use bdk::bitcoin::util::bip32::{ExtendedPrivKey};
use bdk::bitcoin::{Network, Address, Transaction};
use bdk::bitcoincore_rpc::{Client, Auth as core_rpc_auth, RpcApi};
use bdk::blockchain::{ConfigurableBlockchain, LogProgress};
use bdk::database::{MemoryDatabase, BatchDatabase};
use bdk::keys::{ExtendedKey};
use bdk::template::Bip84;
use bdk::wallet::export::WalletExport;
use bdk::keys::bip39::{Mnemonic};
use bdk::{KeychainKind};
use bdk::keys::{DerivableKey};

use bdk::blockchain::rpc::{Auth, RpcBlockchain, RpcConfig};

use bdk::Wallet;
use bdk::wallet::{AddressIndex, signer::SignOptions, wallet_name_from_descriptor};

use std::str::FromStr;

fn get_block_count() -> u64 {
    let rpc_auth = core_rpc_auth::UserPass(
        "admin".to_string(),
        "password".to_string()
    );

    let core_rpc = Client::new("http://127.0.0.1:38332/".into(), rpc_auth).unwrap();

    let block_count = core_rpc.get_block_count();

    match block_count {
        Ok(count) => return count,
        Err(error) => panic!("Problem getting blockchain info: {:?}", error),
    };
}

fn load_or_create_wallet(network: &Network, rpc_url: &str, username: &str, password: &str, xpriv: &ExtendedPrivKey) -> Wallet<RpcBlockchain, MemoryDatabase> {

    let auth = Auth::UserPass {
        username: username.to_string(),
        password: password.to_string()
    };

    // Use deterministic wallet name derived from descriptor
    let wallet_name = wallet_name_from_descriptor(
        Bip84(xpriv.clone(), KeychainKind::External),
        Some(Bip84(*xpriv, KeychainKind::Internal)),
        *network,
        &Secp256k1::new()
    ).unwrap();

    println!("wallet name: {:?}", wallet_name);

    // Setup the RPC configuration
    let rpc_config = RpcConfig {
        url: rpc_url.to_string(),
        auth,
        network: *network,
        wallet_name: wallet_name.clone(),
        skip_blocks: Some(70_000) // Some(block_count)
    };

    // Use the above configuration to create a RPC blockchain backend
    let blockchain = RpcBlockchain::from_config(&rpc_config).unwrap();

    // Combine everything and finally create the BDK wallet structure
    let wallet = Wallet::new(
        Bip84(xpriv.clone(), KeychainKind::External),
        Some(Bip84(*xpriv, KeychainKind::Internal)),
        *network,
        MemoryDatabase::default(),
        blockchain
    ).unwrap();

    // Sync the wallet
    //let sync_result = wallet.sync(LogProgress, None);
    wallet.sync(LogProgress, None).unwrap();

    wallet
}

pub fn mnemonic_to_xprv(network: &Network, mnemonic_words: &str) -> ExtendedPrivKey {
    // Parse a mnemonic
    let mnemonic  = Mnemonic::parse(mnemonic_words).unwrap();

    // Generate the extended key
    let xkey: ExtendedKey = mnemonic.into_extended_key().unwrap();

    // Get xprv from the extended key
    let xprv = xkey.into_xprv(*network).unwrap();

    xprv
}


pub fn build_signed_tx<B, D: BatchDatabase>(wallet: &Wallet<B, D>, recipient_address: &str, amount: u64, nlocktime: Option<u32>) -> Transaction {
    // Create a transaction builder
    let mut tx_builder = wallet.build_tx();

    let to_address = Address::from_str(recipient_address).unwrap();

    match nlocktime {
        Some(nl) => { tx_builder.nlocktime(nl); },
        None => (),
    }

    // Set recipient of the transaction
    tx_builder.set_recipients(vec!((to_address.script_pubkey(), amount)));

    // Finalise the transaction and extract PSBT
    let (mut psbt, _) = tx_builder.finish().unwrap();

    // Sign the above psbt with signing option
    wallet.sign(&mut psbt, SignOptions::default()).unwrap();

    // Extract the final transaction
    let tx = psbt.extract_tx();

    tx
}

pub fn run(network: Network, rpc_url: &str, username: &str, password: &str, mnemonic_words: &str, nlocktime: Option<u32>) {

    let xpriv = mnemonic_to_xprv(&network, &mnemonic_words);

    let wallet = load_or_create_wallet(&network, rpc_url, username, password, &xpriv);

    println!("mnemonic: {}\n\nrecv desc (pub key): {:#?}\n\nchng desc (pub key): {:#?}",
    mnemonic_words,
    wallet.get_descriptor_for_keychain(KeychainKind::External).to_string(),
    wallet.get_descriptor_for_keychain(KeychainKind::Internal).to_string());

    // Fetch a fresh address to receive coins
    let address = wallet.get_address(AddressIndex::New).unwrap().address;

    println!("new address: {}", address);

    let balance = wallet.get_balance().unwrap();

    println!("balance: {}", balance);

    if balance > 100 {

        let recipient_address = "tb1q766f5v4h9ml8dh99ev5ertg2ysrjz2kkuzq8up";

        let tx = build_signed_tx(&wallet, recipient_address, 5000, nlocktime);

        // Broadcast the transaction
        let tx_id = wallet.broadcast(&tx).unwrap();


        println!("tx id: {}", tx_id.to_string());

    } else {
        println!("Insufficient Funds. Fund the wallet with the address above");
    }

    let export = WalletExport::export_wallet(&wallet, "exported wallet", true)
        .map_err(ToString::to_string)
        .map_err(bdk::Error::Generic).unwrap();

    println!("------\nWallet Backup: {}", export.to_string());
}

fn main() {
    let network = Network::Signet;

    let mnemonic_words = "health lyrics appear aunt either wrist maple hover family episode seven maze";

    let rpc_url = "127.0.0.1:38332";

    let username = "admin";
    let password = "password";

    let block_count = get_block_count() as u32;

    println!("------\nblock_count: {}", block_count);

    run(network, rpc_url, username, password, mnemonic_words, Some(block_count));
}
