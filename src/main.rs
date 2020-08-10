use chrono::{DateTime, Utc};
use clap::Clap;
use glob::glob;
use log::{error, info, trace};
use lzma::compress;
use redis_ts::{AsyncTsCommands, TsOptions};
use rustywow::realm::{AuctionResponse, Realm};
use rustywow::{get_session, Error, Opts, Settings, SubCmd};
use serde::ser::Serialize;
use std::fs::File;
use std::io::Write;
use tokio::time::{delay_for, Duration};

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
        SubCmd::Load => {
            info!(
                "Loading dir {} with {}",
                settings.data_dir, settings.db_host
            );
            let client = redis::Client::open(format!("redis://{}/", settings.db_host))
                .expect("Redis connection failed");
            let mut con = client
                .get_async_connection()
                .await
                .expect("Redis client unavailable");
            for entry in
                glob(&format!("{}/*.json", settings.data_dir)).expect("Failed to glob data_dir")
            {
                match entry {
                    Ok(path) => {
                        //AuctionResponse
                        info!("Processing {:?}", path.display());
                        let in_file =
                            File::open(path.clone()).expect("Could not read auction file");
                        //let mut buffer = Vec::new();
                        // read the whole file
                        //in_file.read_to_end(&mut buffer)?;
                        //let r = flexbuffers::Reader::get_root(buffer.as_slice()).unwrap();
                        let rfc3339 = DateTime::parse_from_rfc3339(
                            path.file_stem().unwrap().to_str().unwrap(),
                        )
                        .expect("Invalid date string from filename");
                        let ar: AuctionResponse =
                            serde_json::from_reader(in_file).expect("Couldn't deserialize");

                        for auc in ar.auctions {
                            trace!("Creating against ts {}: {:?}", rfc3339.timestamp(), auc);
                            let key = format!("auction:{}:{}:{}",
                                 &auc.id.to_string(),
                                 &auc.item.id.to_string(),
                                 &auc.quantity.to_string()
                                 //&auc.time_left);
                            );
                            let mut my_opts = TsOptions::default()
                                .retention_time(600000)
                                .label("auction_id", &auc.id.to_string())
                                .label("item", &auc.item.id.to_string())
                                .label("quantity", &auc.quantity.to_string());
                                //.label("time_left", &format!("{}", &auc.time_left));
                            let create_ts = if let Some(buyout) = auc.buyout {
                                my_opts = my_opts.label("buyout", "1").label("unit_price", "0");
                                con.ts_add_create(
                                        key,
                                        rfc3339.timestamp(),
                                        &buyout.to_string(),
                                        my_opts,
                                    )
                                    .await
                                    .expect("Could not store item");
                            } else {
                                if let Some(unit_price) = auc.unit_price {
                                    my_opts = my_opts.label("buyout", "0").label("unit_price", "1");
                                    con.ts_add_create(
                                            key,
                                            rfc3339.timestamp(),
                                            &unit_price.to_string(),
                                            my_opts,
                                        )
                                        .await
                                        .expect("Could not store item");
                                }
                            }
                        }
                        info!("Finished {:?}", path.display());
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

    let json = File::create(format!("{}/{}.json", data_dir, timestamp.to_string()))?;
    serde_json::to_writer(json, auc)?;

    let mut compressed = File::create(format!("{}/{}.7z", data_dir, timestamp.to_string()))?;
    compressed.write_all(&compress(&serde_json::to_vec(auc)?, 9).expect("Failed to compress"))?;
    info!("Auctions saved {}", timestamp.to_string());
    Ok(())
}
