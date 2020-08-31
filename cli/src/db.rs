use crate::{
    realm::{Auction, AuctionResponse, Realm},
    AsKey,
};
use log::{error, info, trace};
use redis::Connection;
use redis::{Client, RedisError};

pub fn redis_connect(db_host: String) -> Result<(Client, Connection), RedisError> {
    let client: Client = Client::open(format!("redis://{}/", db_host)).unwrap();
    let con = client.get_connection()?;
    Ok((client, con))
}

pub async fn dump_redis_proto(auc: &Auction, ts: i64) -> Result<(), String> {
    let mut opt = String::new();
    let key = &auc.item.to_key();
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

/// `TS.RANGE` for the given key
pub fn get_range<T>(
    con: &mut Connection,
    item_md: &T,
) -> std::result::Result<redis::Value, redis::RedisError>
where
    T: AsKey
{
    redis::cmd("TS.RANGE")
        .arg(item_md.to_key())
        .arg("-".to_string())
        .arg("+".to_string())
        .query::<redis::Value>(con)
}
