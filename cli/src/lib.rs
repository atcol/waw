pub mod actors;
pub mod db;
pub mod realm;

use chrono::{DateTime, Duration, Utc};
use clap::Clap;
use log::info;
use redis::RedisError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
pub struct Settings {
    /// Client access identifier
    pub client_id: String,

    /// Secret
    pub client_secret: String,

    /// The realm id, e.g. 1403 = Draenor
    pub realm_id: u16,

    /// The parent directory for all data
    pub data_dir: String,

    /// The time to delay between re-sync'ing data
    pub delay_mins: u64,

    /// Whether to save to fb or not
    pub save_flexbuffer: bool,

    /// The hostname for the redis database
    pub db_host: String,
}

impl Settings {
    pub fn new() -> Result<Self, config::ConfigError> {
        Self::from("Settings")
    }
    pub fn from(file: &'static str) -> Result<Self, config::ConfigError> {
        let mut settings = config::Config::default();
        settings
            // Add in `./Settings.toml`
            .merge(config::File::with_name(&file))?
            // Add in settings from the environment (with a prefix of APP)
            // E.g. `APP_DEBUG=1 ./target/app` would set the `debug` key
            .merge(config::Environment::with_prefix("WAW"))?;
        settings.try_into()
    }
}

#[derive(Clap, Clone)]
#[clap(version = "1.0", author = "Alex Collins")]
pub struct Opts {
    /// The command
    #[clap(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Clap, Clone)]
pub enum SubCmd {
    /// Continuously download auction house and other game data
    #[clap()]
    Sync(SyncOpts),
    /// Load to a Redis instance using raw protocol messages (with `redis-cli --pipe`)
    Load,
}

#[derive(Clap, Clone)]
pub struct SyncOpts {
    /// Don't load in to the database on-the-fly
    #[clap(short, long)]
    pub no_load: bool,
}

/// An period of authenticated interaction with the battle.net APIs
#[derive(Debug, Clone)]
pub struct Session {
    /// When the session opened, where `start_time + auth.expires_in / 60` = expired_date
    start_time: DateTime<Utc>,

    /// The client identifier
    client_id: String,

    /// Secret
    client_secret: String,

    /// The realm id, e.g. 1403 = Draenor
    realm_id: u16,

    auth: Auth,
}

impl Session {
    pub fn has_expired(&self) -> bool {
        (self.start_time + Duration::seconds(self.auth.expires_in.into())) < Utc::now()
    }

    fn auction_url(&self) -> String {
        let url = format!("https://eu.api.blizzard.com/data/wow/connected-realm/{}/auctions?namespace=dynamic-eu&locale=en_US&access_token={}", self.realm_id, self.auth.access_token);
        info!("url: {:?}", url);
        url
    }
}

/// See https://develop.battle.net/documentation/guides/using-oauth/client-credentials-flow
/// curl -u {client_id}:{client_secret} -d grant_type=client_credentials https://us.battle.net/oauth/token
pub async fn authenticate(
    client_id: String,
    client_secret: String,
) -> Result<Auth, reqwest::Error> {
    let client = reqwest::Client::new();
    let auth = client
        .post("https://eu.battle.net/oauth/token")
        .basic_auth(client_id, Some(client_secret))
        .query(&[("grant_type", "client_credentials")])
        .send()
        .await?
        .text()
        .await?;

    info!("Response: {:?}", auth);
    Ok(serde_json::from_str(&auth).expect("Failed parsing auth response"))
}

/// Authenticate and initiate a `Session`
pub async fn get_session(opts: Settings) -> Result<Session, reqwest::Error> {
    Ok(Session {
        start_time: Utc::now(),
        auth: authenticate(opts.client_id.clone(), opts.client_secret.clone()).await?,
        client_id: opts.client_id,
        client_secret: opts.client_secret,
        realm_id: opts.realm_id,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Auth {
    access_token: String,

    token_type: String,

    expires_in: u32,
    /// Optional scoping parameter e.g. wow.profile
    scope: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Error {
    ApiFailure(String),
    AuctionLookup(&'static str),
    ConfigError(String),
    IOError(String),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::ApiFailure(format!("{:?}", e))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IOError(format!("{:?}", e))
    }
}

impl From<config::ConfigError> for Error {
    fn from(e: config::ConfigError) -> Self {
        Error::ConfigError(format!("Configuration error - {:?}", e))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::IOError(format!("JSON serialisation error - {:?}", e))
    }
}

impl From<lzma::LzmaError> for Error {
    fn from(e: lzma::LzmaError) -> Self {
        Error::IOError(format!("Parsing LZMA error - {:?}", e))
    }
}

impl From<redis::RedisError> for Error {
    fn from(e: redis::RedisError) -> Self {
        Error::IOError(format!("Redis error - {:?}", e))
    }
}
