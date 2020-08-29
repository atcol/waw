use actix::{Actor, Addr, Arbiter, Context, Handler, Message, System};
use chrono::{DateTime, Utc};
use clap::Clap;
use glob::glob;
use log::{error, info, trace};
use lzma::{compress, decompress};
use redis::Connection;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use tokio::stream::{self, StreamExt};
use tokio::sync::mpsc::channel;
use tokio::time::{delay_for, Duration};
use waw::actors::{AuctionRow, StorageActor, StoreAuction};
use waw::db::dump_redis_proto;
use waw::realm::{Auction, AuctionResponse, Realm};
use waw::{get_session, Error, Opts, Settings, SubCmd};

fn main() -> Result<(), Error> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    let sys = System::new("Waw");
    let sa_addr = waw::actors::StorageActor::default().start();
    let settings = Settings::new()?;
    let opts = Opts::parse();

    match opts.cmd {
        SubCmd::Sync(sopts) => {
            actix::run(async move {
                loop {
                    match download_auctions(settings.clone()).await {
                        Err(e) => error!("Failed downloading auctions: {:?}", e),
                        Ok((ar, ts_str)) => {
                            let rfc3339 = DateTime::parse_from_rfc3339(&ts_str)
                                .expect("Invalid date string from filename");
                            info!("Download loop completed: {}", ts_str);

                            if !sopts.no_load && ar.auctions.len() > 0 {
                                for a in ar.best_auctions() {
                                    let item_id = a.item_id.clone();
                                    match sa_addr
                                        .send(StoreAuction {
                                            db_host: settings.db_host.clone(),
                                            auction_row: a,
                                            timestamp: rfc3339.timestamp(),
                                        })
                                        .await
                                    {
                                        Ok(sr) => {
                                            trace!("Storage result for {}: {:?}", item_id, sr);
                                        }
                                        Err(e) => {
                                            error!("Inbox full for {}: {}", item_id, e);
                                        }
                                    };
                                }
                            }
                            info!("Finished: {}", ts_str);
                        }
                    };
                    delay_for(Duration::from_secs(60 * settings.delay_mins)).await;
                }
            })?;
        }
        SubCmd::Load => {
            actix::run(async move {
                info!(
                    "Loading dir {} with {}",
                    settings.data_dir, settings.db_host
                );
                let mut auc_stream = tokio::stream::iter(
                    glob(&format!("{}/*.xz", settings.data_dir)).expect("Cannot glob data_dir"),
                )
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
                            }
                            Some(Err(e)) => {
                                error!("Failed parsing auction: {:?}", e);
                                break;
                            }
                            None => {
                                info!("Stream finished");
                                break;
                            }
                        }
                    }
                });

                while let Some((auc, ts)) = rx.recv().await {
                    if auc.unit_price.is_some() {
                        match dump_redis_proto(&auc, ts).await {
                            Ok(_) => {}
                            Err(e) => {
                                error!("Failed to dump redis data: {}", e);
                            }
                        };
                    }
                }
            })?;
        }
    }
    Ok(())
}

fn valid_path(f: Result<std::path::PathBuf, glob::GlobError>) -> Option<std::path::PathBuf> {
    match f {
        Ok(path) => Some(path),
        Err(e) => {
            error!("Error globbing {:?}", e);
            None
        }
    }
}

fn parse_file(p: std::path::PathBuf) -> Result<(AuctionResponse, i64), Error> {
    info!("Loading {:?}", p.clone().display());
    let in_file = File::open(p.clone()).expect("Could not read auction file");

    let rfc3339 = DateTime::parse_from_rfc3339(p.file_stem().unwrap().to_str().unwrap())
        .expect("Invalid date string from filename");
    let mut s = String::new();
    let mut r = lzma::LzmaReader::new_decompressor(in_file)?;
    r.read_to_string(&mut s)?;
    Ok((
        serde_json::from_str(&s).expect(&format!("Failed to read JSON for {:?}", p)),
        rfc3339.timestamp(),
    ))
}

async fn download_auctions(settings: Settings) -> Result<(AuctionResponse, String), Error> {
    let session = get_session(settings.clone())
        .await
        .expect("Failed to authenticate");
    info!("Loading auctions");
    let auc = session.auctions().await?;
    let ts = archive_auctions(settings.data_dir.clone().to_string(), &auc).await?;
    Ok((auc, ts))
}

async fn archive_auctions(data_dir: String, auc: &AuctionResponse) -> Result<String, Error> {
    info!("Saving auctions to {:?}", data_dir);
    let timestamp = Utc::now().format("%+");
    let json = File::create(format!("{}/{}.json", data_dir, timestamp.to_string()))?;
    serde_json::to_writer(json, auc)?;

    let mut compressed = File::create(format!("{}/{}.xz", data_dir, timestamp.to_string()))?;
    compressed.write_all(&compress(&serde_json::to_vec(auc)?, 9).expect("Failed to compress"))?;
    info!("Auctions saved {}", timestamp.to_string());
    Ok(timestamp.to_string())
}
