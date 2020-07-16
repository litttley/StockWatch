use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::smtp::ConnectionReuseParameters;
use lettre::{SmtpClient, Transport,ClientTlsParameters,ClientSecurity};
use lettre_email::Email;


use clokwerk::{Scheduler, TimeUnits};
// Import week days and WeekDay
use clokwerk::Interval::*;
use reqwest::header;
use reqwest::{Client, Error};
use std::thread;
use std::time::Duration;

use bigdecimal::{BigDecimal, ToPrimitive};
use dotenv;
use futures;
use futures::executor::block_on;
use futures::future;
use futures::stream::{self, StreamExt};
use sqlx;
use sqlx::mysql::MySqlPool;
use std::env;
use std::collections::HashMap;
use async_once::AsyncOnce;
use lazy_static::lazy_static;
use tokio::runtime::Runtime;

const PARALLEL_REQUESTS: usize = 2; //控制并行请求数量，太大容易内存不足


lazy_static! {
    static ref FOO : AsyncOnce<MySqlPool> = AsyncOnce::new(async{
           let database_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set");
let pool =  sqlx::MySqlPool::builder().
    max_size(100).// 连接池上限
    min_size(50).// 连接池下限
    connect_timeout(Duration::from_secs(10)).// 连接超时时间
    max_lifetime(Duration::from_secs(1800)).// 所有连接的最大生命周期
    idle_timeout(Duration::from_secs(600)).// 空闲连接的生命周期
    build(database_url.as_str()).await;
        pool.expect("33")
    });




}

lazy_static! {
static ref MAILMAP: HashMap<&'static str, String> = {
let mail_server_host = dotenv::var("mail_server_host").expect("mail_server_host must be set");
let mail_server_port = dotenv::var("mail_server_port").expect("mail_server_port must be set");
let mail_server_username = dotenv::var("mail_server_username").expect("mail_server_username must be set");
let mail_server_password = dotenv::var("mail_server_password").expect("mail_server_password must be set");
let mail_from_addr = dotenv::var("mail_from_addr").expect("mail_from_addr must be set");
let mail_to_addr = dotenv::var("mail_to_addr").expect("mail_to_addr must be set");
let mail_text = dotenv::var("mail_text").expect("mail_text must be set");
let mail_title = dotenv::var("mail_title").expect("mail_title must be set");
let mut m = HashMap::new();
m.insert("mail_server_host", mail_server_host);
m.insert("mail_server_port", mail_server_port);
m.insert("mail_server_username", mail_server_username);
m.insert("mail_server_password", mail_server_password);
m.insert("mail_from_addr", mail_from_addr);
m.insert("mail_to_addr", mail_to_addr);
m.insert("mail_text", mail_text);
m.insert("mail_title", mail_title);

m
};
}


struct StockWatch {
    id: i64,
    stock_code: String,
    remind_price: BigDecimal,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loop {
        println!("loop");

            let client = Client::new();

            let urls = vec![
                "http://qt.gtimg.cn/q=sh600325",
                "http://qt.gtimg.cn/q=sh601890",
            ];

            let bodies = stream::iter(urls)
                .map(|url| {
                    println!("bbbbb");
                    let client = &client;
                    async move {
                        let resp = client
                            .get(url)
                            .timeout(Duration::from_secs(5))
                            .send()
                            .await?;
                        println!("00000");
                        resp.text().await
                    }
                })
                .buffer_unordered(PARALLEL_REQUESTS);

            bodies
                .for_each(|resp| async {
                    println!("ccccc");
                    match resp {
                        Ok(text) => {
                            println!("接口调用成功");
                            parse_stock_data(text).await
                        }
                        Err(e) => eprintln!("调用接口出错: {}", e),
                    }
                })
                .await;


        thread::sleep(Duration::from_millis(5000));
    }
}

async fn parse_stock_data(content: String) {
    println!("1111");
    let stock_data_vec: Vec<&str> = content.split("~").collect();
    let stock_name_return = stock_data_vec[1];
    let stock_code_retrun = stock_data_vec[2];
    let now_price = stock_data_vec[3];
    let now_price = now_price.parse::<f64>().unwrap_or(0.00);
    println!("222222");
    let pool =    FOO.get().await;
    let watch: sqlx::Result<Vec<StockWatch>> = sqlx::query_as!(
        StockWatch,
        r#"
        select id,stock_code,remind_price from setting
                "#,
    )
    .fetch_all(&pool)
    .await;
    println!("33333");
    if let Ok(stock_watch_array) = watch {
        for stock_watch in stock_watch_array {
            let stock_code = stock_watch.stock_code;
            let remind_price = stock_watch.remind_price;
            println!("实时股票代码:{:?}", stock_code_retrun);
            println!("实时价格:{:?}", now_price);
            println!("数据库数据:{:?}", stock_code);
            println!("数据库数据:{:?}", remind_price.to_f64().unwrap_or(0.00));
            if stock_code_retrun.contains(&stock_code) {
                if now_price >= remind_price.to_f64().unwrap_or(0.00) {
                let mail_content =    format!("股票名称:{}   股票代码:{}    当前价格{}",stock_name_return,stock_code_retrun,now_price);
                    send_mail(mail_content).await;
                    //println!("send_mail");
                }
            }
        }
    }
}

async fn send_mail(mail_content:String) {
    let email = Email::builder()
        // Addresses can be specified by the tuple (email, alias)
        .to(MAILMAP.get("mail_to_addr").unwrap().as_str())
        .from(MAILMAP.get("mail_from_addr").unwrap().as_str())
        .subject(MAILMAP.get("mail_title").unwrap().as_str())
        //.text(MAILMAP.get("mail_text").unwrap().to_string())
        .text(mail_content)
        .build()
        .unwrap();


   let mut mailer = SmtpClient::new_simple(MAILMAP.get("mail_server_host").unwrap())
        .unwrap()

        .smtp_utf8(true)
        .credentials(Credentials::new(
            MAILMAP.get("mail_server_username").unwrap().to_string(),
            MAILMAP.get("mail_server_password").unwrap().to_string(),
        ))
        .authentication_mechanism(Mechanism::Plain)
        .connection_reuse(ConnectionReuseParameters::ReuseUnlimited)
        .transport();
    // Send the email
    let result = mailer.send(email.into());
    mailer.close();
    if result.is_ok() {
        println!("发送成功");
    } else {
        println!("发送失败");
    }
}
