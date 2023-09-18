use toml::Value;
use std::fs;

// Structure to represent the configuration
#[derive(Clone)]
pub struct Config {
    pub rpc: RPC,
    pub threads: Threads,
    pub settings: Settings,
}

#[derive(Clone)]
pub struct RPC {
    pub bnb: String,
    pub opbnb: String,
}

#[derive(Clone)]
pub struct Threads {
    pub number_of_threads: u32,
    pub delay_between_threads: (u64, u64),
}

#[derive(Clone)]
pub struct Settings {
    pub cap_key: String,
    pub delay_action: (u64, u64),

    pub use_zk_bridge: bool,
    pub use_bnb_bridge: bool,

    pub value_bridge_min: f64,
    pub value_bridge_max: f64,
    pub value_ridge_decimal: i32,
    pub max_retries_connect_server: i32,
    pub bnb_gwei: f64,
    pub bnb_gas: i32,
    pub opbnb_gwei: f64,
    pub opbnb_gas: i32,
}

pub fn read_config(path: &str) -> Result<Config, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let value: Value = content.parse().expect("Failed to parse TOML");

    Ok(Config {
        rpc: RPC {
            bnb: value["RPC"]["bnb"].as_str().unwrap().to_string(),
            opbnb: value["RPC"]["opbnb"].as_str().unwrap().to_string(),
        },
        threads: Threads {
            number_of_threads: value["threads"]["number_of_threads"].as_integer().unwrap() as u32,
            delay_between_threads: (
                value["threads"]["delay_between_threads"][0].as_integer().unwrap() as u64,
                value["threads"]["delay_between_threads"][1].as_integer().unwrap() as u64,
            ),
        },
        settings: Settings {
            cap_key: value["settings"]["cap_key"].as_str().unwrap().to_string(),
            delay_action: (
                value["settings"]["delay_action"][0].as_integer().unwrap() as u64,
                value["settings"]["delay_action"][1].as_integer().unwrap() as u64,
            ),
            max_retries_connect_server: value["settings"]["max_retries_connect_server"].as_integer().unwrap() as i32,

            bnb_gwei: value["settings"]["bnb_gwei"].as_float().unwrap(),
            bnb_gas: value["settings"]["bnb_gas"].as_integer().unwrap() as i32,
            opbnb_gwei: value["settings"]["opbnb_gwei"].as_float().unwrap(),
            opbnb_gas: value["settings"]["opbnb_gas"].as_integer().unwrap() as i32,

            use_zk_bridge: value["settings"]["use_zk_bridge"].as_bool().unwrap(),
            use_bnb_bridge: value["settings"]["use_bnb_bridge"].as_bool().unwrap(),
            value_bridge_min: value["settings"]["value_bridge_min"].as_float().unwrap(),
            value_bridge_max: value["settings"]["value_bridge_max"].as_float().unwrap(),
            value_ridge_decimal: value["settings"]["value_ridge_decimal"].as_integer().unwrap() as i32,
        },
    })
}


