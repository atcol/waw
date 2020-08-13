use chrono::{DateTime, Utc};
use clap::Clap;
use glob::glob;
use log::{error, info, trace};
use lzma::compress;
use redis::aio::Connection;
use redis_ts::{AsyncTsCommands, TsCommands, TsOptions};
use std::fs::File;
use std::io::Write;
use tokio::stream::{self, StreamExt};
use tokio::sync::mpsc::channel;
use tokio::time::{delay_for, Duration};
use waw::realm::{Auction, AuctionResponse, Realm};
use waw::{get_session, Error, Opts, Settings, SubCmd};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let settings = Settings::new()?;
    let opts = Opts::parse();
    match opts.cmd {
        SubCmd::Sync(sopts) => {
            info!("Spawning auction thread");
            tokio::spawn(async move {
                let mut count: u32 = 1;
                let max: u32 = sopts.count.unwrap_or(0);
                loop {
                    let iterate = count < max || max == 0;
                    if iterate {
                        match download_auctions(settings.clone()).await {
                            Err(e) => error!("Failed downloading auctions: {:?}", e),
                            _ => info!("Download loop completed"),
                        };
                        delay_for(Duration::from_secs(60 * settings.delay_mins)).await;
                        count += 1;
                    } else {
                        break;
                    }
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
            let mut auc_stream = tokio::stream::iter(glob(&format!("{}/*.json", settings.data_dir))
                .expect("Cannot glob data_dir")) 
                .filter_map(valid_path)
                .map(parse_file);

            let (mut tx, mut rx) = channel(100);

            tokio::spawn(async move {
                loop {
                    match auc_stream.next().await {
                        Some(Ok((ar, ts))) => {
                            info!("Received {}", ts);
                            for auc in ar.auctions {
                                if let Err(_) = tx.send((auc, ts)).await {
                                    error!("Receiver dropped: {:?}", ts);
                                    return;
                                }
                            }
                            info!("Done {}", ts);
                        },
                        Some(Err(e)) => {
                            error!("Failed parsing auction: {:?}", e);
                            break;
                        },
                        None => {
                            info!("Stream finished");
                            break;
                        }
                    }
                }
            });

            while let Some((auc, ts)) = rx.recv().await {
                store_auction(&mut con, ts, &auc).await?;
            }
        }
    }
    Ok(())
}

fn valid_path(f: Result<std::path::PathBuf, glob::GlobError>) -> Option<std::path::PathBuf> {
    match f {
        Ok(path) => {
            Some(path)
        },
        Err(e) => {
            error!("Error globbing {:?}", e);
            None
        }
    }
}

fn parse_file(p: std::path::PathBuf) -> Result<(AuctionResponse, i64), Error> {
    info!("Loading {:?}", p.clone().display());
    let in_file =
        File::open(p.clone()).expect("Could not read auction file");
    let rfc3339 = DateTime::parse_from_rfc3339(
        p.file_stem().unwrap().to_str().unwrap(),
    ).expect("Invalid date string from filename");
    Ok((serde_json::from_reader(in_file).expect("Failure parsing file"), rfc3339.timestamp()))
}

async fn store_auction(
    con: &mut Connection,
    ts: i64,
    auc: &Auction,
) -> Result<(), redis::RedisError> {
    let key = format!("item:{}", &auc.item.id.to_string(),);
    let mut my_opts = TsOptions::default()
        .retention_time(600000)
        .label("auction_id", &auc.id.to_string())
        .label("item", &auc.item.id.to_string())
        .label("quantity", &auc.quantity.to_string());
    if let Some(_) = auc.buyout {
        //my_opts = my_opts.label("buyout", "1").label("unit_price", "0");
        //let x: u64 = con
        //    .ts_add_create(key, ts, &buyout.to_string(), my_opts)
        //    .await
        //    .expect("Could not store item");
        trace!("Ignoring buyout for {}", auc.item.id);
    } else {
        if let Some(unit_price) = auc.unit_price {
            //my_opts = my_opts.label("buyout", "0").label("unit_price", "1");
            let x: u64 = con
                .ts_add_create(key, ts, &unit_price.to_string(), my_opts)
                .await
                .expect("Could not store item");
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
    archive_auctions(settings.data_dir.clone().to_string(), &auc).await?;
    Ok(())
}

async fn archive_auctions(data_dir: String, auc: &AuctionResponse) -> Result<(), Error> {
    info!("Saving auctions to {:?}", data_dir);
    let timestamp = Utc::now().format("%+");
    let json = File::create(format!("{}/{}.json", data_dir, timestamp.to_string()))?;
    serde_json::to_writer(json, auc)?;

    let mut compressed = File::create(format!("{}/{}.7z", data_dir, timestamp.to_string()))?;
    compressed.write_all(&compress(&serde_json::to_vec(auc)?, 9).expect("Failed to compress"))?;
    info!("Auctions saved {}", timestamp.to_string());
    Ok(())
}
