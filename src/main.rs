use chrono::{DateTime, NaiveDateTime, Utc, Datelike, Timelike, Duration, TimeZone};
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use serde_json::Value;
use std::collections::HashMap;
use tokio;
use dotenv::dotenv;
use std::env;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use serde::{Deserialize, Serialize};
use lazy_static::lazy_static;
use std::sync::Mutex;

// Constants
const _TABLE_NAME: &str = "klines";

// Core Structs
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct Kline {
    #[serde(rename = "klines_open_time")]                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             /
    kline_open_time: DateTime<Utc>,
    #[serde(rename = "open_price")]
    open_price: f64,
    #[serde(rename = "high_price")]
    high_price: f64,
    #[serde(rename = "low_price")]
    low_price: f64,
    #[serde(rename = "close_price")]
    close_price: f64,
    #[serde(rename = "volume")]
    volume: f64,
    #[serde(rename = "quote_asset_volume")]
    quote_asset_volume: f64,
    #[serde(rename = "klines_close_time")]
    kline_close_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TradingConfig {
    symbol: String,
    base_asset: String,
    quote_asset: String,
    trade_amount: f64,
    order_type: String,
    risk_management: RiskManagement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RiskManagement {
    stop_loss_percentage: f64,
    take_profit_percentage: f64,
    max_daily_loss_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TradeRecord {
    entry_price: f64,
    quantity: f64,
    side: String,
    entry_time: DateTime<Utc>,
}

// Global Trade Tracking
lazy_static! {
    static ref OPEN_TRADES: Mutex<Vec<TradeRecord>> = Mutex::new(Vec::new());
    static ref DAILY_LOSS: Mutex<f64> = Mutex::new(0.0);
}

// Default Configuration Implementation
impl Default for TradingConfig {
    fn default() -> Self {
        TradingConfig {
            symbol: "BTCUSDT".to_string(),
            base_asset: "BTC".to_string(),
            quote_asset: "USDT".to_string(),
            trade_amount: 0.0002,
            order_type: "MARKET".to_string(),
            risk_management: RiskManagement {
                stop_loss_percentage: 2.0,
                take_profit_percentage: 3.0,
                max_daily_loss_percentage: 5.0,
            },
        }
    }
}

// Environment and Configuration Utility Functions
fn get_supabase_url() -> String {
    env::var("SUPABASE_URL").expect("SUPABASE_URL not set in .env")
}

fn get_supabase_key() -> String {
    env::var("SUPABASE_KEY").unwrap_or_else(|_| {
        eprintln!("SUPABASE_KEY not set in .env");
        std::process::exit(1);
    })
}

fn get_supabase_headers() -> Result<HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    let api_key = get_supabase_key();
    
    headers.insert("apikey", HeaderValue::from_str(&api_key)?);
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", api_key))?);
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    Ok(headers)
}

// Risk Management Functions
fn check_daily_loss_limit(config: &TradingConfig, trade_value: f64) -> Result<bool, Box<dyn std::error::Error>> {
    let mut daily_loss = DAILY_LOSS.lock().unwrap();
    
    let potential_total_loss = *daily_loss + trade_value.abs();
    
    if potential_total_loss > config.risk_management.max_daily_loss_percentage {
        println!("Daily loss limit would be exceeded. Trade blocked.");
        return Ok(false);
    }
    
    *daily_loss += trade_value.abs();
    Ok(true)
}

fn check_stop_loss_take_profit(
    current_price: f64, 
    trade_record: &TradeRecord, 
    config: &TradingConfig
) -> Option<String> {
    let stop_loss_price = match trade_record.side.as_str() {
        "BUY" => trade_record.entry_price * (1.0 - config.risk_management.stop_loss_percentage / 100.0),
        "SELL" => trade_record.entry_price * (1.0 + config.risk_management.stop_loss_percentage / 100.0),
        _ => return None,
    };

    let take_profit_price = match trade_record.side.as_str() {
        "BUY" => trade_record.entry_price * (1.0 + config.risk_management.take_profit_percentage / 100.0),
        "SELL" => trade_record.entry_price * (1.0 - config.risk_management.take_profit_percentage / 100.0),
        _ => return None,
    };

    if (trade_record.side == "BUY" && current_price <= stop_loss_price) || 
       (trade_record.side == "SELL" && current_price >= stop_loss_price) {
        return Some("STOP_LOSS".to_string());
    }

    if (trade_record.side == "BUY" && current_price >= take_profit_price) || 
       (trade_record.side == "SELL" && current_price <= take_profit_price) {
        return Some("TAKE_PROFIT".to_string());
    }

    None
}

// Trade Execution Functions
async fn execute_advanced_trade(
    api_client: &Client, 
    config: &TradingConfig,
    signal: &str,
    current_price: f64
) -> Result<(), Box<dyn std::error::Error>> {
    if !check_daily_loss_limit(config, config.trade_amount * current_price)? {
        return Err("Daily loss limit exceeded".into());
    }

    let api_key = env::var("BINANCE_API_KEY")?;
    let secret_key = env::var("BINANCE_SECRET_KEY")?;

    let endpoint = "https://api.binance.com/api/v3/order";
    let timestamp = Utc::now().timestamp_millis();
    
    let query_string = format!(
        "symbol={}&side={}&type={}&quantity={}&timestamp={}",
        config.symbol, 
        signal, 
        config.order_type,
        config.trade_amount,
        timestamp
    );
    
    let mut mac = Hmac::<Sha256>::new_from_slice(secret_key.as_bytes())?;
    mac.update(query_string.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(&api_key)?);

    let url = format!("{}?{}&signature={}", endpoint, query_string, signature);
    let response = api_client
        .post(&url)
        .headers(headers)
        .send()
        .await?;

    if response.status().is_success() {
        println!("{} trade executed: {} {} at price {}", 
            signal, config.trade_amount, config.symbol, current_price);
        
        let mut trades = OPEN_TRADES.lock().unwrap();
        trades.push(TradeRecord {
            entry_price: current_price,
            quantity: config.trade_amount,
            side: signal.to_string(),
            entry_time: Utc::now(),
        });

        Ok(())
    } else {
        let error_text = response.text().await?;
        Err(format!("Trade failed: {}", error_text).into())
    }
}

// Kline and Market Data Functions
fn get_previous_kline_time_block() -> (i64, i64) {
    let now_utc = Utc::now();
    let current_hour = now_utc.hour();

    let (start_hour, _end_hour) = match current_hour {
        0..=3 => (20, 23),
        4..=7 => (0, 3),
        8..=11 => (4, 7),
        12..=15 => (8, 11),
        16..=19 => (12, 15),
        20..=23 => (16, 19),
        _ => unreachable!(),
    }; 

    let previous_period_start = now_utc
        .with_hour(start_hour).unwrap()
        .with_minute(0).unwrap()
        .with_second(0).unwrap()
        .with_nanosecond(0).unwrap();

    let previous_period_end = previous_period_start + Duration::hours(3) + Duration::minutes(59) + Duration::seconds(59) + Duration::milliseconds(999);

    (previous_period_start.timestamp_millis(), previous_period_end.timestamp_millis())
}
//  Existing code remains the same, just correcting the typos in the get_previous_kline function

async fn get_previous_kline(api_client: &Client, start_time: i64, end_time: i64) -> Result<Option<Kline>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://data-api.binance.vision/api/v3/klines?symbol=BTCUSDT&interval=4h&startTime={}&endTime={}",
        start_time, end_time
    );

    let binance_response = api_client.get(&url).send().await?;
    if binance_response.status().is_success() {
        let binance_data: Vec<Vec<Value>> = binance_response.json().await?;
        if let Some(kline) = binance_data.first() {
            let kline_data = Kline {
                kline_open_time: Utc.timestamp_millis(kline[0].as_i64().unwrap_or(0)),
                kline_close_time: Utc.timestamp_millis(kline[6].as_i64().unwrap_or(0)),
                open_price: kline[1].as_str().unwrap_or("0.0").parse().unwrap_or_default(),
                high_price: kline[2].as_str().unwrap_or("0.0").parse().unwrap_or_default(),
                low_price: kline[3].as_str().unwrap_or("0.0").parse().unwrap_or_default(), 
                close_price: kline[4].as_str().unwrap_or("0.0").parse().unwrap_or_default(),
                volume: kline[5].as_str().unwrap_or("0.0").parse().unwrap_or_default(), 
                quote_asset_volume: kline[7].as_str().unwrap_or("0.0").parse().unwrap_or_default(), 
            };

            println!("Kline Data: {:?}", kline_data);
            Ok(Some(kline_data))
        } else {
            Ok(None)
        }
    } else {
        Err("Failed to fetch Kline data.".into())
    }
}

fn is_current_or_previous_month(kline_time: i64) -> bool {
    let kline_date = Utc.timestamp_millis_opt(kline_time).single().unwrap();
    let now = Utc::now();

    (kline_date.year() == now.year() && kline_date.month() == now.month()) ||
    (kline_date.year() == now.year() && kline_date.month() == (now.month() - 1) % 12)
}

async fn fetch_vah_val(api_client: &Client) -> Result<HashMap<String, f64>, Box<dyn std::error::Error>> {
    let url = format!("{}/rest/v1/Monthly_values?select=vah,val", get_supabase_url());
    let headers = get_supabase_headers()?;
    
    let response = api_client.get(&url)
        .headers(headers)
        .send()
        .await?;

    if response.status().is_success() {
        let data: Vec<Value> = response.json().await?;
        let mut vah_val_map = HashMap::new();

        for item in data {
            if let (Some(vah), Some(val)) = (item.get("vah"), item.get("val")) {
                if let (Some(vah_num), Some(val_num)) = (vah.as_f64(), val.as_f64()) {
                    println!("VAH: {}, VAL: {}", vah_num, val_num);
                    vah_val_map.insert("vah".to_string(), vah_num);
                    vah_val_map.insert("val".to_string(), val_num);
                }
            }
        }

        Ok(vah_val_map)
    } else {
        Err(format!("Failed to fetch data: {}", response.status()).into())
    }
}


async fn compare_with_vah_val(
    kline: &Kline, 
    vah_val: &HashMap<String, f64>,
    api_client: &Client
) -> Result<(), Box<dyn std::error::Error>> {
    let config = TradingConfig::default();

    // Check existing open trades for stop loss/take profit
    let mut trades_to_close = Vec::new();
    {
        let mut trades = OPEN_TRADES.lock().unwrap();
        for (index, trade) in trades.iter().enumerate() {
            if let Some(close_reason) = check_stop_loss_take_profit(kline.close_price, trade, &config) {
                println!("{} triggered for trade: {:?}", close_reason, trade);
                trades_to_close.push(index);
            }
        }

        // Remove closed trades (in reverse order to maintain index integrity)
        for &index in trades_to_close.iter().rev() {
            trades.remove(index);
        }
    }

    // Original VAH/VAL trading logic
    if let (Some(vah), Some(val)) = (vah_val.get("vah"), vah_val.get("val")) {
        if kline.close_price > *vah {
            println!("Trade signal: Buy - Close price above VAH.");
            // Execute trade logic (buy)
            let _ = execute_advanced_trade(api_client, &config, "BUY", kline.close_price).await?;
        } else if kline.close_price < *val {
            println!("Trade signal: Sell - Close price below VAL.");
            // Execute trade logic (sell)
            let _ = execute_advanced_trade(api_client, &config, "SELL", kline.close_price).await?;
        }
    }

    Ok(())
}

// Add the missing async function for inserting kline into Supabase
async fn insert_kline_into_supabase(kline: &Kline, api_client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}/rest/v1/{}", get_supabase_url(), "klines");
    let headers = get_supabase_headers()?;

    println!("Inserting data into Supabase: {}", url);
    println!("Kline to insert: {:?}", kline);

    let response = api_client
        .post(&url)
        .headers(headers)
        .json(kline)
        .send()
        .await?;

    let status = response.status().clone();
    let response_text = response.text().await?;

    println!("Response Status: {}", status);
    println!("Response Body: {}", response_text);

    if status.is_success() {
        println!("Kline data inserted successfully!");
        Ok(())
    } else {
        println!("Failed to insert data into Supabase.");
        Err(format!("Error: {} - {}", status, response_text).into())
    }
}




#[tokio::main]
async fn main() {
    dotenv().ok(); // Load the .env file
    let api_client = Client::new();

    //Fetch the previous kline time block
    let (start_time, end_time) = get_previous_kline_time_block();

    if let Ok(Some(kline)) = get_previous_kline(&api_client, start_time, end_time).await {
        if is_current_or_previous_month(kline.kline_open_time.timestamp_millis()) {
            if let Ok(vah_val) = fetch_vah_val(&api_client).await {
                compare_with_vah_val(&kline, &vah_val, &api_client).await;
                if let Err(e) = insert_kline_into_supabase(&kline, &api_client).await {
                    eprintln!("Error inserting kline: {}", e);
                }
            } else {
                eprintln!("Error fetching VAH/VAL values from Supabase.");
            }
        } else {
            println!("Skipping Kline data: Not from current or previous month.");
        }
    } else {
        println!("No Kline data fetched.");
    }
}