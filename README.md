# Trading Bot

This project is a Rust-based trading bot that fetches kline data f, processes it, and integrates with Supabase for database operations. It generates trade signals based on Value Area High (VAH) and Value Area Low (VAL).

## Features
- Fetches previous kline data from Binance based on the last 4-hour block.
- Checks if kline data already exists in Supabase before saving.
- Only processes kline data from the current month.
- Fetches VAH and VAL values from Supabase (`monthly_values` table).
- Compares kline close price with VAH and VAL to generate trade signals.
- Handles asynchronous API requests using `tokio` and `reqwest`.

## Setup

### Prerequisites
- Rust installed
- Supabase account & project setup
- Binance API key

### Installation
1. Clone the repository:
   ```sh
   git clone <repository-url>
   cd bot_project
   ```
2. Create a `.env` file with the following variables:
   ```env
   BINANCE_API_URL=https://api.binance.com
   SUPABASE_URL=<your-supabase-url>
   SUPABASE_KEY=<your-supabase-key>
   ```
## Future Enhancements
- Improve error handling and logging.
- Develop a UI for monitoring bot activity.

## License
This project is licensed under the MIT License.
