use std::thread;
use std::time::Duration;
use std::collections::HashMap;

use futures::stream::{self, StreamExt};

use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::smtp::ConnectionReuseParameters;
use lettre::{SmtpClient, Transport};
use lettre_email::Email;

use reqwest::Client;

use bigdecimal::{BigDecimal, ToPrimitive};
use sqlx::mysql::MySqlPool;

use async_once::AsyncOnce;
use lazy_static::lazy_static;
use colored::*;

const PARALLEL_REQUESTS: usize = 2; //控制并行请求数量，太大容易内存不足

lazy_static! {
    static ref FOO : AsyncOnce<MySqlPool> = AsyncOnce::new(async{

            let database_url = dotenv::var("DATABASE_URL").expect("数据库连接没有设置!");
            let pool =  sqlx::MySqlPool::builder().
                max_size(100).// 连接池上限
                min_size(50).// 连接池下限
                connect_timeout(Duration::from_secs(10)).// 连接超时时间
                max_lifetime(Duration::from_secs(1800)).// 所有连接的最大生命周期
                idle_timeout(Duration::from_secs(600)).// 空闲连接的生命周期
                build(database_url.as_str()).await;
                    pool.expect("数据库连接池创建失败!")
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
    remind_type: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    /*let urls = vec![
        "http://qt.gtimg.cn/q=sh600325",
        "http://qt.gtimg.cn/q=sh601890",
    ];*/

    loop {

        println!("{}","loop\n".yellow().bold());

        let urls = get_urls().await;
        if urls.is_empty() {

            println!("{}!","没有要监听的股票,请到数据库中设置.....".red().bold());
            thread::sleep(Duration::from_millis(5000));

            continue;
        }

        let bodies = stream::iter(urls)
            .map(|url| {
                let client = &client;
                async move {

                    let resp = client
                        .get(&url)
                        .timeout(Duration::from_secs(5))
                        .send()
                        .await?;
                    resp.text().await

                }
            }).buffer_unordered(PARALLEL_REQUESTS);

        bodies.for_each(|resp| async {
                match resp {
                    Ok(text) => {

                        println!("{}","接口调用成功".green().italic());
                        parse_stock_data(text).await

                    }
                    Err(e) =>  println!("{}{}","接口调用失败".red().italic(),e),
                }
            })
            .await;

        thread::sleep(Duration::from_millis(5000));
    }
}

async fn get_urls() -> Vec<String> {
    let pool = FOO.get().await;

    let watch: sqlx::Result<Vec<StockWatch>> = sqlx::query_as!(
        StockWatch,
        r#"
         select id,stock_code,remind_price ,remind_type from setting where is_closed=?
                "#,
        0
    ).fetch_all(&pool).await;

    if let Ok(stock_watch_array) = watch {

        stock_watch_array
            .iter()
            .map(|stock_watch| format!("http://qt.gtimg.cn/q={}", stock_watch.stock_code))
            .collect::<Vec<String>>()

    } else {

        let urls: Vec<String> = Vec::new();
        urls

    }
}

async fn parse_stock_data(content: String) {
    if content.contains("none_match") {

        println!("{}!","股票格式不对".red().bold());
        return;

    }

    let stock_data_vec: Vec<&str> = content.split('~').collect();
    let flag = if  stock_data_vec[0].contains("sz") {"sz"}  else{"sh"} ;
    let stock_name_return = stock_data_vec[1];
    let stock_code_retrun = stock_data_vec[2];
    let now_price = stock_data_vec[3];
    let now_price = now_price.parse::<f64>().unwrap_or(0.00);
    let select_stock_code =   format!("{}{}",flag,stock_code_retrun);

    let pool = FOO.get().await;
    let watch: sqlx::Result<Vec<StockWatch>> = sqlx::query_as!(
        StockWatch,
        r#"
        select id,stock_code,remind_price,remind_type from setting where is_closed=? and stock_code =?
                "#,
        0,
         select_stock_code

    ).fetch_all(&pool).await;

    if let Ok(stock_watch_array) = watch {

        for stock_watch in stock_watch_array {

            let stock_code = stock_watch.stock_code;
            let remind_price = stock_watch.remind_price;
            let remind_type = stock_watch.remind_type;

            print!("{}    ","实    时".cyan().bold());
            print!("{}  ",stock_name_return);
            print!("{}  ",select_stock_code);
            println!("{:?}",now_price);
            if remind_type.eq("0"){
                print!("{}    ","卖出提醒".cyan().bold());
            }else{
                print!("{}    ","买入提醒".cyan().bold());
            }


            print!("{}  ",stock_name_return);
            print!("{}  ",stock_code);
            println!("{:?}",remind_price.to_f64().unwrap_or(0.00));

            if  remind_type.eq("0") && select_stock_code.contains(&stock_code)  && now_price >= remind_price.to_f64().unwrap_or(0.00) {
                let mail_content = format!("股票名称:{}   股票代码:{}    当前价格{}", stock_name_return, stock_code_retrun, now_price);
                let is_true = send_mail(mail_content, "卖出提醒").await;

                if is_true {
                    let sql = r#"update setting set is_closed = ? where  stock_code=?  "#;
                    let _affect_rows = sqlx::query(sql)
                        .bind(1)
                        .bind(&stock_code)
                        .execute(&pool)
                        .await;
                }
            }
          if  remind_type.eq("1") && select_stock_code.contains(&stock_code)  && now_price <= remind_price.to_f64().unwrap_or(0.00) {
                let mail_content = format!("股票名称:{}   股票代码:{}    当前价格{}", stock_name_return, stock_code_retrun, now_price);
                let is_true = send_mail(mail_content,"买入提醒").await;

                if is_true {

                    let sql = r#"update setting set is_closed = ? where  stock_code=?  "#;
                    let _affect_rows = sqlx::query(sql)
                        .bind(1)
                        .bind(&stock_code)
                        .execute(&pool)
                        .await;

                }

        }
    }
    }
}

async fn send_mail(mail_content: String,mail_title:&str) -> bool {

    let email = Email::builder()
        .to(MAILMAP.get("mail_to_addr").unwrap().as_str())
        .from(MAILMAP.get("mail_from_addr").unwrap().as_str())
        .subject(mail_title)
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

    // 发送邮件
    let result = mailer.send(email.into());
    mailer.close();

    if result.is_ok() {

        println!("{}","邮件通知发送成功!!!!".white().bold());
        true
    } else {

        println!("{}","发送失败".red().blink());
        false
    }
}
