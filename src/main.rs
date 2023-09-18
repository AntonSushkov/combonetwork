use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration,};
use reqwest::{Client, Proxy,};
use serde_json::{json, Value};
use log::{error, info};
use rand::prelude::SliceRandom;
use rand::Rng;
use tokio::sync::Semaphore;
use url::Url;
use std::io;
use isahc::{http, prelude::*};
use isahc::{
    auth::{Authentication, Credentials},
    prelude::*,
    HttpClient,
};
use http::Request;
mod utils;
mod constants;
use constants::*;
use utils::{config,};
use utils::error::MyError;
use crate::utils::config::Config;

fn generate_user_agent() -> String {
    let platforms = vec![
        "Windows NT 6.1; Win64; x64",
        "Windows NT 6.0; Win64; x64",
        "Windows; U; Windows NT 6.1",
    ];

    let browsers = vec![
        ("Gecko", "Firefox", 48..=90),
        ("AppleWebKit/605.1.15 (KHTML, like Gecko) Version", "Safari", 530..=600),
    ];

    let platform = platforms.choose(&mut rand::thread_rng()).unwrap();
    let (engine, browser, versions) = browsers.choose(&mut rand::thread_rng()).unwrap();

    let major_version = rand::thread_rng().gen_range(*versions.start()..*versions.end());
    let minor_version = rand::thread_rng().gen_range(0..1000);

    match *engine {
        "Gecko" => {
            format!(
                "Mozilla/5.0 ({}) {}{}/20100101 {}/{}",
                platform, engine, "", browser, major_version
            )
        }
        "AppleWebKit/605.1.15 (KHTML, like Gecko) Version" => {
            format!(
                "Mozilla/5.0 ({}) {}{}.{} {}/605.1.15",
                platform, engine, major_version, minor_version, browser
            )
        }
        _ => unreachable!(), // This won't happen, but it's good to have for completeness
    }
}

async fn build_client(ip: &str, port: &str, login: &str, pass: &str) -> Result<HttpClient, isahc::Error> {
    let proxy_str = format!("http://{}:{}", ip, port);
    let proxy_uri = proxy_str.parse::<http::Uri>().map_err(|e| {
        let io_error = io::Error::new(io::ErrorKind::InvalidInput, e);
        isahc::Error::from(io_error)
    })?;
    let client = HttpClient::builder()
        .proxy(Some(proxy_uri))
        .proxy_authentication(Authentication::basic())
        .proxy_credentials(Credentials::new(login, pass))
        .cookies()
        .build()?;
    Ok(client)
}

async fn build_web3_client(ip: &str, port: &str, login: &str, pass: &str) -> Result<Client, MyError> {
    let proxy = Proxy::https(format!("http://{}:{}", ip, port))?
        .basic_auth(login, pass);
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(30))
        .build()?;
    Ok(client)
}

async fn ip_test (
    session: &HttpClient,
) -> Result<(), isahc::Error> {
    let ip_test = "https://ip.beget.ru/";
    let mut response = session.get_async(ip_test).await?;
    let content = response.text().await.unwrap_or_else(|_| "Failed to read response".to_string());
    let cleaned_content = content.replace(" ", "")
        .replace("{n", "")
        .replace("\n", "");
    println!("IP: {:?}", cleaned_content);
    
    Ok(())
}

async fn combonetwork (
    session: &HttpClient,
    web3_client: &Client,
    wallet_data_line: &str,
    ds_token: &str,
    tw_token: &str,
    config: &Config,
) -> Result<(), MyError>  {
    let wallet_parts: Vec<&str> = wallet_data_line.split(":").collect();
    let address = wallet_parts[0].to_string();
    let private_key = wallet_parts[1].to_string();

    let tw_parts: Vec<&str> = tw_token.split("; ").collect();
    let auth_token = tw_parts[0].split("=").nth(1).unwrap_or("").to_string();
    let ct0 = tw_parts[1].split("=").nth(1).unwrap_or("").to_string();

    let ua = generate_user_agent();

    ip_test(&session).await.expect("Proxy not work");
    // ------
    let parsed = check_combo(&session, &address, &ua).await?;

    if let Some(data) = parsed.get("data") {
        if data.get("discord_joined").and_then(Value::as_i64) == Some(0) {
            connect_ds(&session, &address, ds_token, &ua, &config).await?;
        }

        if data.get("twitter_followed").and_then(Value::as_i64) == Some(0) {
            connect_tw(&session, &address, &auth_token, &ct0, &ua).await?;
        }

        if data.get("telegram_joined").and_then(Value::as_i64) == Some(0) {
            connect_tg(&session, &address, &ua).await?;
        }
    }
    let _parsed = check_combo(&session, &address, &ua).await?;
    // -----
    let key_without_prefix = if private_key.starts_with("0x") {
        &private_key[2..]
    } else {
        &private_key
    };
    let mint_values = check_mint(&session, &address, &ua).await?;
    if let Some((dummy_id_str, signature)) = mint_values {
        let dummy_id: u64 = dummy_id_str.parse().unwrap_or(0);
        if dummy_id == 0 {
            // return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Insufficient balance")));
            println!("dummy_id = 0: {:?}", dummy_id);
        }
        utils::chain::mint_nft(&key_without_prefix, &address, web3_client, dummy_id, &signature, &config).await;
    }

    Ok(())
}

async fn check_combo (
    session: &HttpClient,
    address: &str,
    ua: &str
) -> Result<Value, isahc::Error> {
    // -------------
    let url0 = format!("https://combonetwork.io/api/user?address={}&chain_id=204", address);

    let request0 = Request::builder()
        .method("GET")
        .uri(&url0)
        .header("authority", "combonetwork.io")
        .header("accept", "application/json, text/plain, */*")
        .header("content-type", "application/json")
        .header("origin", "https://combonetwork.io")
        .header("referer", "https://combonetwork.io/mint")
        .header("sec-ch-ua", " ")
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", " ")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-site")
        .header("user-agent", ua)
        .body(())?;
    let mut response0 = session.send_async(request0).await?;

    let text = response0.text().await.unwrap_or_default();
    let parsed: Value = serde_json::from_str(&text).unwrap_or_default();

    // println!("parsed0: {:?}", parsed);

    Ok(parsed)
}

async fn connect_tg (
    session: &HttpClient,
    address: &str,
    ua: &str,
) -> Result<(), isahc::Error> {
    // -------------
    let url0 = "https://combonetwork.io/api/telegram/join".to_string();
    let mut data: HashMap<String, serde_json::Value> = HashMap::new();
    data.insert("address".to_string(), json!(address));
    data.insert("chain_id".to_string(), json!(204));
    let serialized_data = serde_json::to_string(&data).expect("Failed to serialize data");
    // println!("connect_tg_data: {}", &serialized_data);
    let request0 = Request::builder()
        .method("POST")
        .uri(&url0)
        .header("authority", "combonetwork.io")
        .header("accept", "application/json, text/plain, */*")
        .header("content-type", "application/json")
        .header("origin", "https://combonetwork.io")
        .header("referer", "https://combonetwork.io/mint")
        .header("sec-ch-ua", " ")
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", " ")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-site")
        .header("user-agent", ua)
        .body(serialized_data)?;
    let mut response0 = session.send_async(request0).await?;

    // let text = response0.text().await.unwrap_or_default();
    // let parsed: Value = serde_json::from_str(&text).unwrap_or_default();
    // println!("connect_tg_parsed0: {:?}", parsed);

    Ok(())
}

async fn check_mint (
    session: &HttpClient,
    address: &str,
    ua: &str
) -> Result<Option<(String, String)>, MyError> {
    // -------------
    let url0 = "https://combonetwork.io/api/mint/sign".to_string();
    let mut data: HashMap<String, serde_json::Value> = HashMap::new();
    data.insert("nft_contract".to_string(), json!("0x20Cb10B8f601d4B2C62962BB938554F3824e24f3"));
    data.insert("mint_contract".to_string(), json!("0x514A16EDd7A916efC662d1E360684602fd72DCD7"));
    data.insert("mint_to".to_string(), json!(address));
    data.insert("chain_id".to_string(), json!(204));
    let serialized_data = serde_json::to_string(&data).expect("Failed to serialize data");
    // println!("data: {}", &serialized_data);
    let request0 = Request::builder()
        .method("POST")
        .uri(&url0)
        .header("authority", "combonetwork.io")
        .header("accept", "application/json, text/plain, */*")
        .header("content-type", "application/json")
        .header("origin", "https://combonetwork.io")
        .header("referer", "https://combonetwork.io/mint")
        .header("sec-ch-ua", " ")
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", " ")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-site")
        .header("user-agent", ua)
        .body(serialized_data).map_err(|err| MyError::ErrorStr(format!("HTTP error: {}", err)))?;
    let mut response0 = session.send_async(request0).await.map_err(MyError::IsahcReqwest)?;

    let text = response0.text().await.unwrap_or_default();
    let parsed: Value = serde_json::from_str(&text).unwrap_or_default();

    // println!("parsed0: {:?}", parsed);

    if let Some((dummy_id, signature)) = {
        let dummy_id = parsed["data"]["dummy_id"]
            .as_str()
            .ok_or(MyError::ErrorStr("Failed to extract dummy_id".to_string()))?;

        let signature = parsed["data"]["signature"]
            .as_str()
            .ok_or(MyError::ErrorStr("Failed to extract signature".to_string()))?;

        Some((dummy_id.to_string(), signature.to_string()))
    } {
        info!("| {} |dummy_id: {}, signature: {}", address, dummy_id, signature);
        Ok(Some((dummy_id, signature)))
    } else {
        error!("Failed to extract dummy_id and signature");
        Ok(None)
    }
}

async fn connect_ds (
    session: &HttpClient,
    address: &str,
    ds_token: &str,
    ua: &str,
    config: &Config,
) -> Result<(), MyError> {
    // --------------
    let url_ds = format!("https://combonetwork.io/api/discord/verify?provider=discord&address={}&chain_id=204", address);

    let request_ds = Request::builder()
        .method("GET")
        .uri(&url_ds)
        .header("authority", "combonetwork.io")
        .header("accept", "application/json, text/plain, */*")
        .header("content-type", "application/json")
        .header("origin", "https://combonetwork.io")
        .header("referer", "https://combonetwork.io/mint")
        .header("sec-ch-ua", " ")
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", " ")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-site")
        .header("user-agent", ua)
        .body(()).map_err(|err| MyError::ErrorStr(format!("HTTP error: {}", err)))?;
    let response_ds = session.send_async(request_ds).await?;

    let mut client_id = String::new();
    let mut state = String::new();
    let mut location_str_url = String::new();
    if let Some(location) = response_ds.headers().get(http::header::LOCATION) {
        let location_str = location.to_str().unwrap_or_default();
        location_str_url = location.to_str().unwrap_or_default().parse().unwrap();
        let parsed_url = Url::parse(location_str).unwrap();
        let query_params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        client_id = query_params.get("client_id").cloned().unwrap_or_default();
        state = query_params.get("state").cloned().unwrap_or_default();
        // println!("client_id: {:?}", client_id);
        // println!("state: {:?}", state);
    } else {
        error!("| {} | Conncet DS: No location header found in the response.", address);
    }

    // -------------
    let redirect_uri = "https://combonetwork.io/api/discord/callback?provider=discord";
    let scope = "identify+guilds+guilds.members.read";
    let url1 = format!("https://discord.com/api/v9/oauth2/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}", client_id, redirect_uri, scope, state );

    let request1 = Request::builder()
        .method("GET")
        .uri(&url1)
        .header("authority", "discord.com")
        .header("authorization", ds_token)
        .header("content-type", "application/json")
        .header("referer", &location_str_url)
        .header("x-super-properties", X_SUPER_PROPERTIES)
        .body(()).map_err(|err| MyError::ErrorStr(format!("HTTP error: {}", err)))?;
    let _response1 = session.send_async(request1).await?;

    let mut data: HashMap<String, serde_json::Value> = HashMap::new();
    data.insert("permissions".to_string(), json!("0"));
    data.insert("authorize".to_string(), json!(true));

    let serialized_data = serde_json::to_string(&data).expect("Failed to serialize data");
    // println!("Serialized JSON Data: {}", serialized_data);
    let request2 = Request::builder()
        .method("POST")
        .uri(&url1)
        .header("authority", "discord.com")
        .header("authorization", ds_token)
        .header("content-type", "application/json")
        .header("referer", &location_str_url)
        .header("x-super-properties", X_SUPER_PROPERTIES)
        .body(serialized_data).map_err(|err| MyError::ErrorStr(format!("HTTP error: {}", err)))?;
    let mut response2 = session.send_async(request2).await?;
    let content = response2.text().await.unwrap_or_else(|_| "Failed to read response".to_string());

    let parsed_content: Result<serde_json::Value, _> = serde_json::from_str(&content);
    match parsed_content {
        Ok(content) => {
            if let Some(location_url) = content["location"].as_str() {
                let _parsed_url = Url::parse(location_url).unwrap();
                let _loc_url = session.get_async(&location_url.to_string()).await?;

                let invite = "rxR6vrz3DT";
                let result = utils::discord::join_server(session, ds_token, invite, &ua, &config).await;

                match result {
                    Ok(_) => {},
                    Err(e) if e.to_string() == "Failed Join after max attempts" => {
                        // println!("Error with discord connection");
                        return Err(MyError::ErrorStr("Error with discord connection".to_string()))
                    },
                    Err(e) => return Err(utils::error::MyError::IsahcReqwest(e)),
                }

                info!("| {} | Connect DS: {:?} | {} |", address, result, ds_token)

            }
        },
        Err(e) => {
            error!("| {} | Error connect DS: Failed to parse JSON content: {}", address, e);
        }
    }

    Ok(())
}

async fn connect_tw (
    session: &HttpClient,
    address: &str,
    auth_token: &str,
    ct0: &str,
    ua: &str
) -> Result<(), isahc::Error> {

    let url_ds = format!("https://combonetwork.io/api/twitter/verify?address={}&chain_id=204", address);

    let request_tw = Request::builder()
        .method("GET")
        .uri(&url_ds)
        .header("authority", "combonetwork.io")
        .header("accept", "application/json, text/plain, */*")
        .header("content-type", "application/json")
        .header("origin", "https://combonetwork.io")
        .header("referer", "https://combonetwork.io/mint")
        .header("sec-ch-ua", " ")
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", " ")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-site")
        .header("user-agent", ua)
        .body(())?;
    let response_tw = session.send_async(request_tw).await?;


    let mut location_str_url = String::new();
    if let Some(location) = response_tw.headers().get(http::header::LOCATION) {
        let _location_str = location.to_str().unwrap_or_default();
        location_str_url = location.to_str().unwrap_or_default().parse().unwrap();
        // println!("location_str_url: {:?}", location_str_url);
        let result = utils::twitter::connect_oauth2(session, &location_str_url, auth_token, ct0, &ua).await;

        info!("| {} | Connect TW: {:?}", address, result)
    } else {
        error!("| {} | Error connect TW: No location header found in the response.", address);
    }

    Ok(())
}

async fn random_delay(range: (u64, u64)) {
    let (min, max) = range;
    let delay_duration = rand::thread_rng().gen_range(min..=max);
    tokio::time::sleep(tokio::time::Duration::from_secs(delay_duration)).await;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the logger
    utils::logger::setup_logger().unwrap();

    // Read config
    let arc_config = Arc::new(config::read_config("Config/Config.toml").expect("Failed to read config"));

    // Read files
    let proxy_lines = std::fs::read_to_string("FILEs/proxy.txt")?;
    let wallet_data_lines = std::fs::read_to_string("FILEs/address_private_key.txt")?;
    let ds_tokens_lines = std::fs::read_to_string("FILEs/ds_token.txt")?;
    let tw_tokens_lines = std::fs::read_to_string("FILEs/tw_token.txt")?;

    let paired_data: Vec<_> = proxy_lines.lines().map(String::from)
        .zip(wallet_data_lines.lines().map(String::from))
        .zip(ds_tokens_lines.lines().map(String::from))
        .zip(tw_tokens_lines.lines().map(String::from))
        .map(|(((proxy, wallet), ds_token), tw_token)| (proxy, wallet, ds_token, tw_token))
        .collect();

    let max_concurrent_tasks = arc_config.threads.number_of_threads;  // Adjusted
    let semaphore = Arc::new(Semaphore::new(max_concurrent_tasks as usize));

    let futures: Vec<_> = paired_data.into_iter().enumerate().map(|(index, (proxy_line, wallet_data_line, ds_token, tw_token))| {
        let proxy_parts: Vec<String> = proxy_line.split(":").map(|s| s.to_string()).collect();

        let (ip, port, login, pass) = (proxy_parts[0].clone(), proxy_parts[1].clone(), proxy_parts[2].clone(), proxy_parts[3].clone());

        let sema_clone = semaphore.clone();
        let config_clone = arc_config.clone();


        tokio::spawn(async move {
            if index > 0 {
                random_delay(config_clone.threads.delay_between_threads).await;  // Add this at the beginning of the thread
            }

            // Acquire semaphore permit
            let _permit = sema_clone.acquire().await;

            let client = match build_client(&ip, &port, &login, &pass).await {
                Ok(c) => c,
                Err(e) => {
                    error!("| | Failed to build client: {}", e.to_string());
                    return;
                }
            };

            let web3_client = match build_web3_client(&ip, &port, &login, &pass).await {
                Ok(c) => c,
                Err(e) => {
                    error!("| | Failed to build web3 client: {}", e.to_string());
                    return;
                }
            };

            combonetwork(&client, &web3_client, wallet_data_line.as_str(), &ds_token, &tw_token, &config_clone).await;

        })
    }).collect();

    futures::future::join_all(futures).await;

    Ok(())
}