use chrono::Utc;
use clap::Clap;
use glob::glob;
use log::{info, error};
use lzma::compress;
use redis::AsyncCommands;
use redis_ts::{AsyncTsCommands, TsCommands, TsOptions};
use rustywow::realm::{AuctionResponse, Realm};
use rustywow::{get_session, Error, Opts, Session, Settings, SubCmd};
use serde::ser::{Serialize};
use serde::{Deserialize};
use std::fs::File;
use std::io::Read;
use std::io::Write;
use tokio::time::{delay_for, Duration};
use tokio_timer::*;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let settings = Settings::new()?;
    let opts = Opts::parse();
    match opts.cmd {
        SubCmd::Sync => {
            info!("Spawning auction thread");
            tokio::spawn(async move {
                loop {
                    match download_auctions(settings.clone()).await {
                        Err(e) => error!("Failed downloading auctions: {:?}", e),
                        _ => info!("Download loop completed"),
                    };
                    delay_for(Duration::from_secs(60 * settings.delay_mins)).await;
                }
            })
            .await;
        }
        SubCmd::Load(db) => {
            info!("Sync'ing dir {} with {:?}", settings.data_dir, db);
            let client = redis::Client::open(format!("redis://{}/", db.host))
                .expect("Redis connection failed");
            let mut con = client
                .get_async_connection()
                .await
                .expect("Redis client unavailable");
            for entry in
                glob(&format!("{}/*.fb", settings.data_dir)).expect("Failed to glob data_dir")
            {
                match entry {
                    Ok(path) => {
                        //AuctionResponse
                        info!("Processing {:?}", path.display());
                        let mut in_file = File::open(path).expect("Could not read auction file");
                        let mut buffer = Vec::new();
                        // read the whole file
                        in_file.read_to_end(&mut buffer)?;
                        let r = flexbuffers::Reader::get_root(buffer.as_slice()).unwrap();
                        let key = db.schema.clone();
                        for auc in AuctionResponse::deserialize(r)
                                .expect("Couldn't deserialize").auctions {
                            //TODO if buyout, append buyout label and use that val, otherwise use
                            //unit_price
                            let my_opts = TsOptions::default()
                                .retention_time(600000)
                                .label("item", &auc.item.id.to_string())
                                .label("quantity", &auc.quantity.to_string())
                                .label("auction_id", &auc.id.to_string());
                                //.label("", &format!("{:?}", auc.time_left)),
                                //.label("buyout", auc.buyout)
                                //.label("quantity", &auc.quantity.to_string())
                                //.label("time_left", auc.time_left.to_str())
                            let create_ts: u64 = con
                                .ts_add_create(
                                    key.clone(), 
                                    "*",
                                    &auc.buyout.unwrap_or(0).to_string(),
                                    my_opts)
                                .await.expect("Could not store item");
                        }
                    }
                    Err(e) => println!("{:?}", e),
                }
            }
        }
    }
    Ok(())
}

async fn download_auctions(settings: Settings) -> Result<(), Error> {
    let session = get_session(settings.clone())
        .await
        .expect("Failed to authenticate");
    info!("Loading auctions");
    let auc = session.auctions().await?;
    save_auctions(settings.data_dir.clone().to_string(), &auc).await?;
    Ok(())
}

async fn save_auctions(data_dir: String, auc: &AuctionResponse) -> Result<(), Error> {
    info!("Saving auctions");
    let mut s = flexbuffers::FlexbufferSerializer::new();
    auc.serialize(&mut s).unwrap();
    let timestamp = Utc::now().format("%+");
    let mut raw = File::create(format!("{}/{}.fb", data_dir, timestamp.to_string()))?;
    raw.write_all(s.view())?;

    let mut compressed = File::create(format!("{}/{}.7z", data_dir, timestamp.to_string()))?;
    compressed.write_all(&compress(s.view(), 9).expect("Failed to compress"))?;
    info!("Auctions saved {}", timestamp.to_string());
    Ok(())
}
