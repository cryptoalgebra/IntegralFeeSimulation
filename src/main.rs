extern crate serde;

use arbiter_core::{
    environment::{BlockSettings, EnvironmentParameters, GasSettings},
    manager::Manager,
    middleware::RevmMiddleware,
};
use ethers::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

use std::{error::Error, sync::Arc};

use crate::bindings::simulation_adaptive_fee::SimulationAdaptiveFee;
use anyhow::Result;

mod bindings;

const TEST_ENV_LABEL: &str = "test";

const TIMEOUT: u64 = 60 * 60;

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

    let _ = manager.add_environment(EnvironmentParameters {
        label: TEST_ENV_LABEL.to_owned(),
        block_settings: BlockSettings::RandomlySampled {
            block_rate: 1.0,
            block_time: 1,
            seed: 1,
        },
        gas_settings: GasSettings::Constant(0),
    });
    manager.start_environment(TEST_ENV_LABEL)?;

    let client_with_signer = Arc::new(
        RevmMiddleware::new(manager.environments.get(TEST_ENV_LABEL).unwrap(), None).unwrap(),
    );

    let oracle_simulation = SimulationAdaptiveFee::deploy(client_with_signer.clone(), ())
        .unwrap()
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

    let mut gas_cumulative = U256::zero();
    let mut last_pack_gas_cumulative = U256::zero();
    let mut counter = 0;
    let mut last_pack_counter = 0;

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

            let gas_used = tx.unwrap().gas_used.unwrap();
            gas_cumulative += gas_used;
            counter += 1;

            res.push(ResultOfSwap {
                timestamp,
                tickAverage: fee_data.2,
                volatilityAverage: fee_data.1,
                tick: last_tick,
                fee: fee_data.0,
                gasUsed: gas_used.to_string(),
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
                    "Done {:?} / {:?}, avg gas:{:?}, est time: {:?}:{:?}:{:?}",
                    index,
                    swaps_events.len(),
                    (gas_cumulative - last_pack_gas_cumulative) / (counter - last_pack_counter),
                    estimated_hours,
                    estimated_minutes,
                    estimated_seconds
                );
            } else {
                println!("Done {:?} / {:?}", index, swaps_events.len());
            }

            pack_num = index as u32 / 5000;
            last_pack_gas_cumulative = gas_cumulative;
            last_pack_counter = counter;

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

    let mut file = std::fs::File::create("./output/result.json")?;
    serde_json::to_writer(&mut file, &res)?;
    Ok(())
}

fn get_time_now() -> u64 {
    return SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}
