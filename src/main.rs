extern crate serde;

use arbiter_core::{
    environment::EnvironmentParameters, manager::Manager, middleware::RevmMiddleware,
};
use ethers::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

use std::{error::Error, sync::Arc, time::Duration};

use crate::bindings::simulation_adaptive_fee::SimulationAdaptiveFee;
use anyhow::Result;
use ethers::{
    core::{k256::ecdsa::SigningKey, utils::Anvil},
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer, Wallet},
    utils::AnvilInstance,
};

mod bindings;

const TEST_ENV_LABEL: &str = "test";

const TIMEOUT: u64 = 60 * 60;
const USE_ANVIL: bool = false;

#[derive(Debug, Deserialize)]
struct SwapEvent {
    timestamp: String,
    tick: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[allow(non_snake_case)]
struct ResultOfSwap {
    timestamp: u32,
    tickAverage: i32,
    fee: u16,
    volatilityAverage: u128,
    tick: i32,
    gasUsed: String,
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let mut manager = Manager::new();

    let _ = manager.add_environment(
        TEST_ENV_LABEL,
        EnvironmentParameters {
            block_rate: 1.0,
            seed: 1,
        },
    );

    let res: Vec<ResultOfSwap>;
    if USE_ANVIL {
        let (client_with_signer, _anvil_instance) = anvil_startup().await?;
        println!("Anvil started");
        res = simulator(client_with_signer).await?;
    } else {
        let client_with_signer = Arc::new(RevmMiddleware::new(
            manager.environments.get(TEST_ENV_LABEL).unwrap(),
            None,
        ));
        manager.start_environment(TEST_ENV_LABEL)?;
        res = simulator(client_with_signer).await?;
    }

    let mut file = std::fs::File::create("./output/result.json")?;
    serde_json::to_writer(&mut file, &res)?;
    Ok(())
}

async fn simulator<M: Middleware + 'static>(client: Arc<M>) -> Result<Vec<ResultOfSwap>> {
    let oracle_simulation = SimulationAdaptiveFee::deploy(client.clone(), ())?
        .send()
        .await?;

    println!(
        "Simulation contract deployed {:?}",
        oracle_simulation.address()
    );

    let mut file = File::open("./input/swaps.json").unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    let swaps_events: Vec<SwapEvent> =
        serde_json::from_str(&data).expect("JSON was not well-formatted");

    let mut pack_num: u32 = 0;
    let mut res: Vec<ResultOfSwap> = vec![];

    let start_time = get_time_now();
    println!("Start time {:?}", start_time);

    let mut last_tick: i32 = swaps_events[0].tick.parse::<i32>().unwrap();
    let mut last_timestamp: u32 = swaps_events[0].timestamp.parse::<u32>().unwrap();
    println!("First tick {:?}", last_tick);

    oracle_simulation
        .init(last_tick, last_timestamp)
        .send()
        .await?
        .await?;

    for index in 1..swaps_events.len() {
        let timestamp = swaps_events[index].timestamp.parse::<u32>().unwrap();
        let tick = swaps_events[index].tick.parse::<i32>().unwrap();
        if last_timestamp != timestamp {
            let time_delta = timestamp - last_timestamp;

            let tx: Option<TransactionReceipt> = oracle_simulation
                .update(
                    bindings::simulation_adaptive_fee::simulation_adaptive_fee::UpdateParams {
                        advance_time_by: time_delta,
                        tick: last_tick,
                    },
                )
                .send()
                .await?
                .await?;

            last_timestamp = timestamp;
            let fee_data = oracle_simulation.get_fee().await?;

            let mut gas_used = "0".to_string();

            if USE_ANVIL {
                gas_used = tx.unwrap().gas_used.unwrap().to_string();
            }

            res.push(ResultOfSwap {
                timestamp,
                tickAverage: fee_data.2,
                volatilityAverage: fee_data.1,
                tick: last_tick,
                fee: fee_data.0,
                gasUsed: gas_used,
            })
        }
        last_tick = tick;

        if index as u32 / 5000 != pack_num {
            let time_now = get_time_now();
            let speed = index as f64 / (time_now - start_time) as f64;
            if speed > 0.0 {
                let time_estimation = ((swaps_events.len() - index) as f64 / speed).floor() as i64;
                let estimated_hours = time_estimation / (60 * 60);
                let estimated_minutes = time_estimation % (60 * 60) / 60;
                let estimated_seconds = time_estimation % (60 * 60) % 60;

                println!(
                    "Done {:?} / {:?} est time: {:?}:{:?}:{:?}",
                    index,
                    swaps_events.len(),
                    estimated_hours,
                    estimated_minutes,
                    estimated_seconds
                );
            } else {
                println!("Done {:?} / {:?}", index, swaps_events.len());
            }

            pack_num = index as u32 / 5000;

            if time_now - start_time > TIMEOUT {
                println!(
                    "Finishing by timeout: {:?} / {:?}",
                    index,
                    swaps_events.len()
                );
                break;
            }
        }
    }

    let finish_time = get_time_now();
    println!("Finish time {:?}", finish_time);
    println!("Elapsed (seconds) {:?}", finish_time - start_time);

    Ok(res)
}

async fn anvil_startup() -> Result<(
    Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    AnvilInstance,
)> {
    // Create an Anvil instance
    // No blocktime mines a new block for each tx, which is fastest.
    let anvil = Anvil::new().spawn();

    // Create a client
    let provider = Provider::<Http>::try_from(anvil.endpoint())
        .unwrap()
        .interval(Duration::ZERO);

    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));

    Ok((client, anvil))
}

fn get_time_now() -> u64 {
    return SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}
