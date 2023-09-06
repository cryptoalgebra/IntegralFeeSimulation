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

use std::{error::Error, sync::Arc};

use crate::bindings::adaptive_fee_simulation::AdaptiveFeeSimulation;

mod bindings;

const TEST_ENV_LABEL: &str = "test";

#[derive(Debug, Deserialize)]
struct SwapEvent {
    timestamp: String,
    tick: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ResultOfSwap {
    timestamp: u32,
    tickAverage: i32,
    fee: u16,
    volatilityAverage: u128,
    tick: i32,
}

fn get_time_now() -> u64 {
    return SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}

const TIMEOUT: u64 = 60 * 60;

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

    let client_with_signer = Arc::new(RevmMiddleware::new(
        manager.environments.get(TEST_ENV_LABEL).unwrap(),
        None,
    ));
    manager.start_environment(TEST_ENV_LABEL)?;

    let oracle_simulation = AdaptiveFeeSimulation::deploy(client_with_signer.clone(), ())?
        .send()
        .await?;

    println!(
        "Simulation contract deployed {:?}",
        oracle_simulation.address()
    );

    let mut file = File::open("./input/swaps.json").unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    let json: Vec<SwapEvent> = serde_json::from_str(&data).expect("JSON was not well-formatted");

    let mut pack_num: u32 = 0;
    let mut res: Vec<ResultOfSwap> = vec![];

    let start_time = get_time_now();
    println!("Start time {:?}", start_time);

    let mut last_tick: i32 = json[0].tick.parse::<i32>().unwrap();
    let mut last_timestamp: u32 = json[0].timestamp.parse::<u32>().unwrap();
    println!("First tick {:?}", last_tick);

    oracle_simulation
        .init(last_tick, last_timestamp)
        .send()
        .await?
        .await?;

    for index in 1..100 {
        let timestamp = json[index].timestamp.parse::<u32>().unwrap();
        let tick = json[index].tick.parse::<i32>().unwrap();
        if last_timestamp != timestamp {
            let time_delta = timestamp - last_timestamp;

            let _: Option<TransactionReceipt> = oracle_simulation
                .update(
                    bindings::adaptive_fee_simulation::adaptive_fee_simulation::UpdateParams {
                        advance_time_by: time_delta,
                        tick: last_tick,
                    },
                )
                .send()
                .await?
                .await?;

            last_timestamp = timestamp;
            let fee_data = oracle_simulation.get_fee().await?;

            res.push(ResultOfSwap {
                timestamp,
                tickAverage: fee_data.2,
                volatilityAverage: fee_data.1,
                tick: last_tick,
                fee: fee_data.0,
            })
        }
        last_tick = tick;

        if index as u32 / 5000 != pack_num {
            let time_now = get_time_now();
            let speed = index as f64 / (time_now - start_time) as f64;
            if speed > 0.0 {
                let time_estimation = ((json.len() - index) as f64 / speed).floor() as i64;
                let estimated_hours = time_estimation / (60 * 60);
                let estimated_minutes = time_estimation % (60 * 60) / 60;
                let estimated_seconds = time_estimation % (60 * 60) % 60;

                println!(
                    "Done {:?} / {:?} est time: {:?}:{:?}:{:?}",
                    index,
                    json.len(),
                    estimated_hours,
                    estimated_minutes,
                    estimated_seconds
                );
            } else {
                println!("Done {:?} / {:?}", index, json.len());
            }

            pack_num = index as u32 / 5000;

            if time_now - start_time > TIMEOUT {
                println!("Finishing by timeout: {:?} / {:?}", index, json.len());
                break;
            }
        }
    }

    let mut file = std::fs::File::create("./output/result.json")?;
    serde_json::to_writer(&mut file, &res)?;

    let finish_time = get_time_now();
    println!("Finish time {:?}", finish_time);
    println!("Elapsed (seconds) {:?}", finish_time - start_time);

    Ok(())
}
