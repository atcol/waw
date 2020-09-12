use crate::AsKey;
use actix::{Actor, Context, Handler, Message, System};
use log::{error, info, trace};

#[derive(Debug)]
/// Mandatory data for auction storage
pub struct AuctionRow {
    pub item_id: u64,
    pub auction_id: u64,
    pub quantity: u16,
    pub unit_price: u64,
}

impl AsKey for AuctionRow {
    fn id(&self) -> String {
        self.item_id.to_string()
    }

    fn prefix(&self) -> Option<String> {
        Some("items".to_string())
    }
}

pub struct StorageActor;

impl Default for StorageActor {
    fn default() -> Self {
        Self {}
    }
}

impl Actor for StorageActor {
    type Context = Context<Self>;
}
#[derive(Debug, Message)]
#[rtype(result = "StorageResult")]
pub struct StoreAuction {
    pub db_host: String,
    pub auction_row: AuctionRow,
    pub timestamp: i64,
}

#[derive(Debug, actix::MessageResponse)]
pub enum StorageResult {
    Failed(String),
    Success,
}

impl Handler<StoreAuction> for StorageActor {
    type Result = StorageResult;

    fn handle(&mut self, msg: StoreAuction, ctx: &mut Self::Context) -> Self::Result {
        match crate::db::redis_connect(msg.db_host) {
            Err(e) => {
                error!("Failed to connect to redis: {}", e);
                StorageResult::Failed(format!("Redis connection error: {}", e))
            }
            Ok((c, mut con)) => {
                let c: &mut dyn redis::ConnectionLike = &mut con;
                trace!("Storing: {:?}", msg.auction_row);
                match crate::db::store_auction(
                    &mut con,
                    msg.auction_row.to_key(),
                    msg.timestamp,
                    msg.auction_row.unit_price,
                    msg.auction_row.auction_id.to_string(),
                    msg.auction_row.item_id,
                    msg.auction_row.quantity,
                ) {
                    Ok(_) => {
                        trace!("Stored {}", msg.auction_row.item_id);
                        StorageResult::Success
                    }
                    Err(e) => {
                        error!("Failed to store item {:?}: {}", msg.auction_row, e);
                        StorageResult::Failed(format!("Redis error: {}", e))
                    }
                }
            }
        }
    }
}
