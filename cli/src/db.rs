use crate::realm::{Auction, AuctionResponse, Realm};
use log::{error, info, trace};
use redis::Connection;
use redis::{Client, RedisError};
use redis_ts::{TsOptions};

pub fn redis_connect(db_host: String) -> Result<(Client, Connection), RedisError> {
    let client: Client = Client::open(format!("redis://{}/", db_host)).unwrap();
    let con = client.get_connection()?;
    Ok((client, con))
}

pub async fn dump_redis_proto(auc: &Auction, ts: i64) -> Result<(), String> {
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
