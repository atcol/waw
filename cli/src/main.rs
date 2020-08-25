use actix::{Actor, Addr, Arbiter, Context, Handler, Message, System};
use chrono::{DateTime, Utc};
use clap::Clap;
use glob::glob;
use itertools::Itertools;
use log::{error, info, trace};
use lzma::compress;
use redis::Connection;
use redis::{Client, RedisError};
use redis_ts::{TsCommands, TsOptions};
use std::fs::File;
use std::io::Write;
use tokio::stream::{self, StreamExt};
use tokio::sync::mpsc::channel;
use tokio::time::{delay_for, Duration};
use waw::realm::{Auction, AuctionResponse, Realm};
use waw::{get_session, Error, Opts, Settings, SubCmd};

#[derive(Debug)]
/// Mandatory data for auction storage
pub struct AuctionRow {
    item_id: u64,
    auction_id: u64, 
    quantity: u16, 
    unit_price: u64,
}

struct StorageActor;

impl Default for StorageActor {
    fn default() -> Self {
        Self {
        }
    }
}

impl Actor for StorageActor {
    type Context = Context<Self>;
}
#[derive(Debug, Message)]
#[rtype(result = "StorageResult")]
struct StoreAuction(String, AuctionRow, i64);

#[derive(Debug, actix::MessageResponse)]
enum StorageResult {
    Failed(String),
    Success
}

impl Handler<StoreAuction> for StorageActor {
    type Result = StorageResult;

    fn handle(&mut self, msg: StoreAuction, ctx: &mut Self::Context) -> Self::Result {
        match redis_connect(msg.0) {
            Err(e) => {
                error!("Failed to connect to redis: {}", e);
                StorageResult::Failed(format!("Redis connection error: {}", e))
            },
            Ok((c, mut con)) => {
                info!("Storing: {:?}", msg.1);
                match store_auction(&mut con, msg.2, msg.1.auction_id, msg.1.item_id, msg.1.quantity, msg.1.unit_price)
                {
                    Ok(_) => {
                        trace!("Stored {}", msg.1.item_id);
                        StorageResult::Success
                    },
                    Err(e) => {
                        error!("Failed to store {}", e);
                        StorageResult::Failed(format!("Redis error: {}", e))
                    },
                }
            }
        }
    }
}

fn main() -> Result<(), Error> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    let sys = System::new("Waw");
    let sa_addr = StorageActor::default().start();
    let settings = Settings::new()?;
    let opts = Opts::parse();

    match opts.cmd {
        SubCmd::Sync(sopts) => {
            info!("Spawning auction thread");
            actix::run(async move {
                loop {
                    match download_auctions(settings.clone()).await {
                        Err(e) => error!("Failed downloading auctions: {:?}", e),
                        Ok((ar, ts_str)) => {
                            let rfc3339 = DateTime::parse_from_rfc3339(&ts_str)
                                .expect("Invalid date string from filename");
                            info!("Download loop completed: {}", ts_str);

                            if !sopts.no_load && ar.auctions.len() > 0 {
                                for a in ar.auctions.iter()
                                    .filter(|a| a.unit_price.is_some())
                                    .sorted_by_key(|x| x.unit_price.unwrap())
                                    .rev()
                                    .group_by(|x| x.item.id)
                                    .into_iter()
                                    .map(|(iid, au)| (iid, au.take(1).next().unwrap()))
                                    .map(|(iid, au)| AuctionRow { item_id: iid, auction_id: au.id, quantity: au.quantity, unit_price: au.unit_price.unwrap() }) {
                                        let item_id = a.item_id.clone();
                                        match sa_addr.send(StoreAuction(settings.db_host.clone(), a, rfc3339.timestamp())).await {
                                            Ok(sr) => {
                                                trace!("Storage result for {}: {:?}", item_id, sr);
                                            },
                                            Err(e) => {
                                                error!("Inbox full for {}: {}", item_id, e);
                                            },
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
                    glob(&format!("{}/*.json", settings.data_dir)).expect("Cannot glob data_dir"),
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
                            Ok(_) => {},
                            Err(e) => {
                                error!("Failed to dump redis data: {}", e);
                            },
                        };
                    }
                }
            })?;
        }
    }
    Ok(())
}

async fn dump_redis_proto(auc: &Auction, ts: i64) -> Result<(), String> {
    let mut opt = String::new();
    let key = format!("item:{}", &auc.item.id.to_string());
    let auc_id = &auc.item.id.to_string();
    let item_id = &auc.item.id.to_string();
    let quant = &auc.quantity.to_string();
    let val = &auc.unit_price.unwrap().to_string();
    let tss = &ts.to_string();
    let cmd = vec![
        "TS.ADD",
        &key,
        tss,
        val,
        "labels",
        "auction_id",
        auc_id,
        "item",
        item_id,
        "quantity",
        quant,
    ];
    opt.push_str(&format!("*{}\r\n", cmd.len()));
    for arg in cmd {
        opt.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
    }
    println!("{}", opt);
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
    Ok((
        serde_json::from_reader(in_file).expect("Failure parsing file"),
        rfc3339.timestamp(),
    ))
}

fn store_auction(
    con: &mut Connection,
    ts: i64,
    auc_id: u64,
    item_id: u64,
    quantity: u16,
    unit_price: u64,
) -> Result<(), redis::RedisError> {
    let key = format!("item:{}", &item_id.to_string(),);
    let my_opts = TsOptions::default()
        .retention_time(600000)
        .label("auction_id", &auc_id.to_string())
        .label("item", &item_id.to_string())
        .label("quantity", &quantity.to_string());
    info!("Storing {}", key);
    let _: u64 = con
        .ts_add_create(key, ts, &unit_price.to_string(), my_opts)
        .expect("Could not store item");
    Ok(())
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

    let mut compressed = File::create(format!("{}/{}.7z", data_dir, timestamp.to_string()))?;
    compressed.write_all(&compress(&serde_json::to_vec(auc)?, 9).expect("Failed to compress"))?;
    info!("Auctions saved {}", timestamp.to_string());
    Ok(timestamp.to_string())
}

fn redis_connect(db_host: String) -> Result<(Client, Connection), RedisError> {
    let client: Client = Client::open(format!("redis://{}/", db_host)).unwrap();
    let con = client.get_connection()?;
    Ok((client, con))
}
