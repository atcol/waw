use crate::{Error, Session};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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
                println!("{:?}", ahd.auctions.len());
                Ok(ahd)
            }
            sc => {
                println!("Unexpected response status code: {:?}", sc);
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
    auctions: Vec<Auction>,
}

/// An individual auction
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Auction {
    id: u64,
    item: Item,
    buyout: Option<u64>,
    quantity: u16,
    time_left: AuctionTime,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum AuctionTime {
    SHORT,
    MEDIUM,
    LONG,
    VERY_LONG,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    id: u64,
    context: Option<u16>,
}
