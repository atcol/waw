use crate::realm::{Auction, AuctionResponse, Realm};
use log::{error, info, trace};
use redis::Connection;
use redis::{Client, RedisError};
use redis_ts::{TsCommands, TsOptions};

pub fn redis_connect(db_host: String) -> Result<(Client, Connection), RedisError> {
    let client: Client = Client::open(format!("redis://{}/", db_host)).unwrap();
    let con = client.get_connection()?;
    let mut c = client.get_connection()?;
    Ok((client, con))
}

pub fn store_auction(
    con: Connection,
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
    trace!("Storing {}", key);
    // let _: u64 = con
    //     .ts_add_create(key, ts, &unit_price.to_string(), my_opts)
    //     .expect("Could not store item");
    Ok(())
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
