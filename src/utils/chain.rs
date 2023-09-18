use log::{error, info};
use rand::Rng;
use reqwest::Client;
use secp256k1::{SecretKey};
use serde_json::{Value};
use tokio::time::{sleep, Duration, Instant};
use web3::{
    Web3, transports::Http, types::{Address, U256, U64, TransactionParameters}
};
use crate::{
    constants::*,
    MyError,
    utils::{file_manager::append_to_file,
            config::{Config}},
};


pub fn generate_web3_clients(client: Client, config: &Config,) -> (Web3<Http>, Web3<Http>) {
    let opbnb_http = Http::with_client(client.clone(), (&config.rpc.opbnb).parse().unwrap());
    let web3_opbnb = Web3::new(opbnb_http);

    let bnb_http = Http::with_client(client.clone(), (&config.rpc.bnb).parse().unwrap());
    let web3_bnb = Web3::new(bnb_http);

    (web3_opbnb, web3_bnb)
}



pub async fn mint_nft(private_key: &str, address: &str, client: &Client, dummy_id: u64, signature: &str, config: &Config,) {
    let (web3_opbnb, web3_bnb) = generate_web3_clients(client.clone(), &config);

    check_and_log_balance(&web3_opbnb, &address, "OpBNB").await;
    check_and_log_balance(&web3_bnb, &address, "BNB").await;

    // Bridge from BNB to OpBNB via opbnb-bridge
    if config.settings.use_bnb_bridge {
        match bnb_bridge_bnb_opbnb(&private_key, &address, &web3_bnb, &config, client).await {
            Ok(_c) => info!("| {} | opbnb-bridge - Ok", address),
            Err(e) => {
                error!("| {} | Failed to opbnb-bridge: {}", address, e.to_string());
                // return;
            }
        }
        random_delay(config.settings.delay_action).await;
    }


    // Bridge from BNB to OpBNB via zkbridge
    if config.settings.use_zk_bridge {
        match zk_bridge_bnb_opbnb(&private_key, &address, &web3_bnb, &config, client).await {
            Ok(_c) => info!("| {} | zk-bridge - Ok", address),
            Err(e) => {
                error!("| {} | Failed to zk-bridge: {}", address, e.to_string());
                // return;
            }
        }
        random_delay(config.settings.delay_action).await;
    }


    // Mint NFT
    match mint(&private_key, &address, &web3_opbnb, dummy_id, signature, &config).await {
        Ok(_c) => info!("| {} | mint - Ok", address),
        Err(e) => {
            error!("| {} | Failed to mint: {}", address, e.to_string());
            // return;
        }
    }

}



async fn bnb_bridge_bnb_opbnb(private_key: &str, address_str: &str, web3: &Web3<Http>, config: &Config, client: &Client) -> Result<(), Box<dyn std::error::Error>> {

    let address: Address = address_str.parse().expect("Failed to parse address");

    let random_value = rand::thread_rng().gen_range(config.settings.value_bridge_min..config.settings.value_bridge_max);
    let final_amount = (random_value * 10f64.powi(config.settings.value_ridge_decimal.clone() as i32)).round() / 10f64.powi(config.settings.value_ridge_decimal.clone() as i32);
    let bridge_amount: U256 = U256::from((final_amount * 1e18) as u64);

    let bnb_balance = web3.eth().balance(address, None).await?;
    if bnb_balance < bridge_amount {
        error!("bnb_balance: {:?} < bridge_amount: {}", bnb_balance, bridge_amount);
        return Err(Box::new(MyError::ErrorStr("Insufficient BNB balance".to_string())));
    }

    let bnb_bridge: Address = BNB_BRIDGE.parse().expect("Failed to parse Ethereum address");

    let data = bnb_generate_transfer_data();
    let data_bytes = hex::decode(&data[2..]).expect("Failed to decode hex string to bytes");
    // let gas_price: U256 = web3.eth().gas_price().await.expect("Failed to fetch gas price");

    let nonce = web3.eth().transaction_count(address, None).await?;

    // let gwei_in_wei: U256 = U256::from(1_000_000_000); // 1 * 10^9
    let gwei_in_wei: U256 = U256::from_dec_str(&format!("{:.0}", config.settings.bnb_gwei * 10f64.powi(9))).unwrap();

    let txn_parameters = TransactionParameters {
        nonce: Some(nonce),
        to: Some(bnb_bridge),
        value: U256::from(bridge_amount),
        gas_price: Some(gwei_in_wei),
        gas: U256::from(config.settings.bnb_gas),
        data: data_bytes.into(),
        chain_id: None,
        transaction_type: None,
        access_list: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    // println!("txn_parameters: {:?}", txn_parameters);


    let key_bytes = hex::decode(&private_key).expect("Failed to decode hex");
    let secret_key = SecretKey::from_slice(&key_bytes).expect("Invalid private key bytes");
    let signed_txn = web3.accounts().sign_transaction(txn_parameters, &secret_key).await?;

    let _ = sleep(Duration::from_secs(2));

    let tx_hash = web3.eth().send_raw_transaction(signed_txn.raw_transaction).await?;

    match wait_until_tx_finished(&web3, tx_hash, 360).await {
        Ok((success, returned_tx_hash)) => {
            if success {
                info!("| {} | opbnb-bridge Transaction was successful! https://bscscan.com/tx/{:?}", &address_str, returned_tx_hash);
                let _ = sleep(Duration::from_secs(10));
            } else {
                error!("| {} | opbnb-bridge Transaction failed! https://bscscan.com/tx/{:?}", &address_str, returned_tx_hash);
            }
        },
        Err(err) => error!("Error: {}", err),
    }

    let tx_hash_str = format!("{:?}", tx_hash);
    wait_for_bridge_completion(&tx_hash_str, address_str, client.clone()).await;

    Ok(())
}


async fn zk_bridge_bnb_opbnb(private_key: &str, address_str: &str, web3: &Web3<Http>, config: &Config, client: &Client) -> Result<(), Box<dyn std::error::Error>> {

    let address: Address = address_str.parse().expect("Failed to parse address");

    let random_value = rand::thread_rng().gen_range(config.settings.value_bridge_min..config.settings.value_bridge_max);
    let final_amount = (random_value * 10f64.powi(config.settings.value_ridge_decimal.clone() as i32)).round() / 10f64.powi(config.settings.value_ridge_decimal.clone() as i32);
    let bridge_amount: U256 = U256::from((final_amount * 1e18) as u64);
    let fee_amount: U256 = U256::from_dec_str(&format!("{:.0}", 0.001 * 10f64.powi(18))).unwrap();
    let value: U256 = bridge_amount + fee_amount;

    let bnb_balance = web3.eth().balance(address, None).await?;
    if bnb_balance < value {
        error!("bnb_balance: {:?} < value: {}", bnb_balance, value);
        return Err(Box::new(MyError::ErrorStr("Insufficient BNB balance".to_string())));
    }

    let zk_bridge: Address = ZK_BRIDGE.parse().expect("Failed to parse Ethereum address");

    let data = generate_transfer_data(bridge_amount, address_str);
    let data_bytes = hex::decode(&data[2..]).expect("Failed to decode hex string to bytes");

    // let gas_price: U256 = web3.eth().gas_price().await.expect("Failed to fetch gas price");

    let nonce = web3.eth().transaction_count(address, None).await?;

    // let gwei_in_wei: U256 = U256::from(1_000_000_000); // 1 * 10^9
    let gwei_in_wei: U256 = U256::from_dec_str(&format!("{:.0}", config.settings.bnb_gwei * 10f64.powi(9))).unwrap();

    let txn_parameters = TransactionParameters {
        nonce: Some(nonce),
        to: Some(zk_bridge),
        value: U256::from(value),
        gas_price: Some(gwei_in_wei),
        gas: U256::from(config.settings.bnb_gas),
        data: data_bytes.into(),
        chain_id: None,
        transaction_type: None,
        access_list: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    // println!("txn_parameters: {:?}", txn_parameters);


    let key_bytes = hex::decode(&private_key).expect("Failed to decode hex");
    let secret_key = SecretKey::from_slice(&key_bytes).expect("Invalid private key bytes");
    let signed_txn = web3.accounts().sign_transaction(txn_parameters, &secret_key).await?;

    let _ = sleep(Duration::from_secs(2));

    let tx_hash = web3.eth().send_raw_transaction(signed_txn.raw_transaction).await?;

    match wait_until_tx_finished(&web3, tx_hash, 360).await {
        Ok((success, returned_tx_hash)) => {
            if success {
                info!("| {} | zk-bridge Transaction was successful! https://bscscan.com/tx/{:?}", &address_str, returned_tx_hash);
                let _ = sleep(Duration::from_secs(10));
            } else {
                error!("| {} | zk-bridge Transaction failed! https://bscscan.com/tx/{:?}", &address_str, returned_tx_hash);
            }
        },
        Err(err) => error!("Error: {}", err),
    }

    let tx_hash_str = format!("{:?}", tx_hash);
    wait_for_bridge_completion(&tx_hash_str, address_str, client.clone()).await;

    Ok(())
}


async fn mint(private_key: &str, address_str: &str, web3: &Web3<Http>, dummy_id: u64, signature: &str, config: &Config,)  -> Result<(), Box<dyn std::error::Error>> {
    let address: Address = address_str.parse().expect("Failed to parse address");


    let opbnb_balance = web3.eth().balance(address, None).await?;
    if opbnb_balance == U256::zero() {
        error!("Insufficient OpBNB balance: {}", opbnb_balance);
        return Err(Box::new(MyError::ErrorStr("Insufficient OpBNB balance".to_string())));
    }

    let mint_contract: Address = MINT_CONTRACT.parse().expect("Failed to parse Ethereum address");
    let nft_contract = NFT_CONTRACT;

    let data = generate_mint_data(nft_contract, dummy_id, address_str, signature);
    let data_bytes = hex::decode(&data[2..]).expect("Failed to decode hex string to bytes");

    // let gas_price: U256 = web3.eth().gas_price().await.expect("Failed to fetch gas price");

    let nonce = web3.eth().transaction_count(address, None).await?;

    // let gwei_in_wei: U256 = U256::from(500_000_000); // 0.5 * 10^9
    let gwei_in_wei: U256 = U256::from_dec_str(&format!("{:.0}", &config.settings.opbnb_gwei * 10f64.powi(9))).unwrap();

    let txn_parameters = TransactionParameters {
        nonce: Some(nonce),
        to: Some(mint_contract),
        value: U256::zero(),
        gas_price: Some(gwei_in_wei),
        gas: U256::from(config.settings.opbnb_gas),
        data: data_bytes.into(),
        chain_id: None,
        transaction_type: None,
        access_list: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
    };
    // println!("txn_parameters: {:?}", txn_parameters);


    let key_bytes = hex::decode(&private_key).expect("Failed to decode hex");
    let secret_key = SecretKey::from_slice(&key_bytes).expect("Invalid private key bytes");
    let signed_txn = web3.accounts().sign_transaction(txn_parameters, &secret_key).await?;

    let _ = sleep(Duration::from_secs(2));

    let tx_hash = web3.eth().send_raw_transaction(signed_txn.raw_transaction).await?;

    match wait_until_tx_finished(&web3, tx_hash, 360).await {
        Ok((success, returned_tx_hash)) => {
            let data_tr = format!("{} | {}", &address_str, &tx_hash);
            let folder = "result".to_string();
            if success {
                info!("| {} | MINT: Transaction was successful! https://opbnbscan.com/tx/{:?}", &address_str, returned_tx_hash);
                append_to_file(&data_tr, &folder).await.expect("Error write data in file 'result.txt'");
            } else {
                error!("| {} | MINT: Transaction failed! https://opbnbscan.com/tx/{:?}", &address_str, returned_tx_hash);
                append_to_file(&data_tr, &folder).await.expect("Error write data in file 'result.txt'");
            }
        },
        Err(err) => error!("Error: {}", err),
    }

    Ok(())
}



async fn random_delay(range: (u64, u64)) {
    let (min, max) = range;
    let delay_duration = rand::thread_rng().gen_range(min..=max);
    tokio::time::sleep(tokio::time::Duration::from_secs(delay_duration)).await;
}

fn bnb_generate_transfer_data() -> String {
    // Method ID
    let method_id = "b1a1a882";

    // Convert min_gas_limit to hexadecimal and pad to 64 characters (32 bytes)
    let min_gas_limit = format!("{:064x}", 200000);

    // _extra_data
    let _extra_data = "00000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000";

    // Concatenate all components
    format!("0x{}{}{}", method_id, min_gas_limit, _extra_data)
}

fn generate_transfer_data(amount: U256, address: &str) -> String {
    // Method ID
    let method_id = "14d9e096";

    // Convert dst_chain_id to hexadecimal and pad to 64 characters (32 bytes)
    let dst_chain_id_hex = format!("{:064x}", 23);

    // Convert amount to hexadecimal and pad to 64 characters (32 bytes)
    let amount_hex = format!("{:064x}", amount);

    // Convert recipient to hexadecimal and prepend zeros to make it 64 characters (32 bytes)
    let recipient = if address.starts_with("0x") {
        &address[2..]
    } else {
        &address
    };
    let recipient_hex = format!("000000000000000000000000{}", recipient);

    // Concatenate all components
    format!("0x{}{}{}{}", method_id, dst_chain_id_hex, amount_hex, recipient_hex)
}

fn generate_mint_data(
    nft: &str,
    dummy_id: u64,
    mint_to: &str,
    signature: &str
) -> String {
    let method_id = "b5fd9ec5";

    // Convert nft address to hexadecimal and pad to 64 characters (32 bytes)
    let nft_address = if nft.starts_with("0x") { &nft[2..] } else { nft };
    let nft_padded = format!("000000000000000000000000{}", nft_address);

    // Convert dummy_id to hexadecimal and pad to 64 characters (32 bytes)
    let dummy_id_hex = format!("{:064x}", dummy_id);

    // Convert mint_to address to hexadecimal and pad to 64 characters (32 bytes)
    let mint_address = if mint_to.starts_with("0x") { &mint_to[2..] } else { mint_to };
    let mint_to_padded = format!("000000000000000000000000{}", mint_address);

    // Split the signature into two parts for proper formatting
    let signat = if signature.starts_with("0x") { &signature[2..] } else { signature };
    let (signature_part1, signature_part2) = signat.split_at(64);

    format!(
        "0x{}{}{}{}00000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000041{}{}00000000000000000000000000000000000000000000000000000000000000",
        method_id, nft_padded, dummy_id_hex, mint_to_padded, signature_part1, signature_part2
    )
}

async fn wait_until_tx_finished(web3: &Web3<Http>, tx_hash: web3::types::H256, max_wait_secs: u64) -> Result<(bool, web3::types::H256), &'static str> {
    let start_time = Instant::now();
    let max_wait_time = Duration::from_secs(max_wait_secs);

    while start_time.elapsed() < max_wait_time {
        match web3.eth().transaction_receipt(tx_hash).await {
            Ok(Some(receipt)) => {
                let one = U64::from(1);
                match receipt.status {
                    Some(status) if status == one => {
                        // println!("Transaction was successful! {:?}", tx_hash);
                        return Ok((true, tx_hash));
                    },
                    Some(_) => {
                        error!("Transaction failed! {:?}", receipt);
                        return Ok((false, tx_hash));
                    },
                    None => {
                        tokio::time::sleep(Duration::from_millis(300)).await;
                    },
                }
            },
            Ok(None) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            },
            Err(_) => {
                if start_time.elapsed() > max_wait_time {
                    error!("FAILED TX: {:?}", tx_hash);
                    return Ok((false, tx_hash));
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    Err("Reached maximum wait time without transaction confirmation.")
}

async fn check_and_log_balance(web3: &Web3<Http>, address: &str, network_name: &str) {
    match check_balance(web3, address).await {
        Ok(balance) => {
            info!("| {} | {}", address, format!(
                    "Balance: {} {}",
                    format_balance_to_float(&balance).to_string(),
                    network_name.to_string()));
        },
        Err(e) => {
            eprintln!("Failed to check balance on {}: {}", network_name, e);
        }
    }
}

async fn check_balance(web3: &Web3<Http>, address: &str) -> web3::Result<U256> {
    match address.parse::<Address>() {
        Ok(address_h160) => web3.eth().balance(address_h160, None).await,
        Err(_) => {
            println!("Failed to parse address: {}", address);
            Err(web3::Error::InvalidResponse("Failed to parse address".into()))
        }
    }
}

fn format_balance_to_float(value: &U256) -> f64 {
    value.as_u128() as f64 / 1_000_000_000_000_000_000.0
}

async fn check_status_bridge(tx_hash: &str, address: &str, client: &Client) -> Result<bool, reqwest::Error> {
    let url = format!("https://op-bnb-mainnet-explorer-api.nodereal.io/api/tx/getAssetTransferByAddress?address={}&pageSize=20&page=1&type=deposit", address);
    let response: Value = client.get(&url).send().await?.json().await?;
    // println!("response Value: {:?}", response);

    if let Some(l1_tx_hash) = response["data"]["list"][0]["l1TxHash"].as_str() {
        if l1_tx_hash == tx_hash {
            let receipts_status = response["data"]["list"][0]["receiptsStatus"].as_i64().unwrap_or_default();
            if receipts_status == 1 {
                // println!("Transaction was successful!");
                Ok(true)
            } else {
                // println!("Transaction failed with status {}", receipts_status);
                Ok(false)
            }
        } else {
            // println!("l1TxHash does not match the provided tx_hash");
            Ok(false)
        }
    } else {
        // eprintln!("Failed to extract l1TxHash from the response");
        Ok(false)
    }
}


async fn wait_for_bridge_completion(tx_hash: &str, address: &str, client: Client) {
    let start_time = Instant::now();
    let max_wait_time = Duration::from_secs(600);
    loop {
        if start_time.elapsed() >= max_wait_time {
            error!("Reached maximum wait time without transaction confirmation.");
            break;
        }

        match check_status_bridge(tx_hash, address, &client).await {
            Ok(false) => {
                println!("Bridge is not yet complete...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            },
            Ok(true) => {
                println!("Bridge has completed!");
                break;
            },
            Err(e) => {
                eprintln!("Error while checking: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}