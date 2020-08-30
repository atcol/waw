use crate::AsKey;
use crate::{Error, Session};
use async_trait::async_trait;
use itertools::Itertools;
use log::info;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A WoW realm
#[async_trait]
pub trait Realm {
    async fn auctions(&self) -> Result<AuctionResponse, Error>;
}

#[async_trait]
impl Realm for Session {
    async fn auctions(&self) -> Result<AuctionResponse, Error> {
        let aurl = &self.auction_url();
        let res = reqwest::get(aurl).await?;
        match res.status() {
            reqwest::StatusCode::OK => {
                let ahd: AuctionResponse = res.json().await?;
                info!("{:?}", ahd.auctions.len());
                Ok(ahd)
            }
            sc => {
                info!("Unexpected response status code: {:?}", sc);
                Err(Error::AuctionLookup("Auction look-up failed"))
            }
        }
    }
}

/// A link to the realm details
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectedRealmLink {
    href: String,
}

/// The parent type for all auctions
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct AuctionResponse {
    connected_realm: ConnectedRealmLink,
    pub auctions: Vec<Auction>,
}

impl AuctionResponse {
    /// List the auctions by their lowest price
    pub fn best_auctions(&self) -> Vec<crate::actors::AuctionRow> {
        self.auctions
            .iter()
            .filter(|a| a.unit_price.is_some())
            .sorted_by_key(|x| x.unit_price.unwrap())
            .rev()
            .group_by(|x| x.item.id)
            .into_iter()
            .map(|(iid, au)| (iid, au.take(1).next().unwrap()))
            .map(|(iid, au)| crate::actors::AuctionRow {
                item_id: iid,
                auction_id: au.id,
                quantity: au.quantity,
                unit_price: au.unit_price.unwrap(),
            })
            .collect()
    }
}

/// An individual auction
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Auction {
    pub id: u64,
    pub item: ItemIden,
    pub buyout: Option<u64>,
    pub unit_price: Option<u64>,
    pub quantity: u16,
    pub time_left: AuctionTime,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum AuctionTime {
    SHORT,
    MEDIUM,
    LONG,
    VERY_LONG,
}

impl fmt::Display for AuctionTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemIden {
    pub id: u64,
    context: Option<u16>,
}

impl AsKey for ItemIden {
    fn id(&self) -> String {
        self.id.to_string()
    }

    fn prefix(&self) -> Option<String> {
         Some("item".to_string()) 
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub en_us: String,
}
