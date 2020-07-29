use chrono::Utc;
use clap::Clap;
use lzma::compress;
use rustywow::realm::{AuctionResponse, Realm};
use rustywow::{get_session, Settings, Error, Opts, Session, SubCmd};
use serde::ser::Serialize;
use std::fs::File;
use std::io::Write;
use tokio::time::{delay_for, Duration};
use tokio_timer::*;
use log::{info,};

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
                    run(settings.clone()).await;
                    delay_for(Duration::from_secs(60 * settings.delay_mins)).await;
                }
            })
            .await;
        },
        SubCmd::Load(db) => {
            info!("Loading to {} from {}", db.pg_string, settings.data_dir);      
        },
    }
    Ok(())
}

async fn run(settings: Settings) -> Result<(), Error> {
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
