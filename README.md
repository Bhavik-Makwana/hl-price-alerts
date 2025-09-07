# Hyperliquid Price Alert Bot

A Telegram bot that monitors cryptocurrency prices on the Hyperliquid exchange and sends real-time price alerts to users when their target prices are reached.

## Features

- 🔔 **Real-time Price Monitoring**: Continuously monitors cryptocurrency prices using Hyperliquid's WebSocket API
- 📱 **Telegram Integration**: Sends instant notifications via Telegram when price targets are hit
- 💾 **Persistent Storage**: SQLite database to store user alerts and settings
- ⏰ **Cooldown System**: Prevents spam by implementing cooldown periods for triggered alerts
- 🎯 **Multi-token Support**: Monitor multiple cryptocurrencies simultaneously
- 🔄 **Auto-reset**: Automatically resets alert cooldowns for future triggers
- ⏰ **Cron Alerts**: Schedule daily alerts that fire at specific times (e.g., 8am daily)

## Architecture

The application consists of several key components:

- **AlertService**: Manages alert creation, retrieval, and triggering logic
- **CronService**: Handles scheduled alerts and cron job management
- **NotificationService**: Handles Telegram bot commands and message sending
- **Database**: SQLite-based storage for alerts, cron alerts, and user data
- **WebSocket Client**: Real-time price monitoring via Hyperliquid API
- **Cron Worker**: Background task that triggers scheduled alerts at specified times


### Telegram Commands

- `/help` - Display available commands
- `/alert` - View all your current price alerts
- `/setalert <coin> <price>` - Create a new price alert
  - Example: `/setalert HYPE 100.0`
- `/cronalerts` - View all your scheduled cron alerts
- `/setcronalert <message>` - Create a daily cron alert at 8am
  - Example: `/setcronalert Good morning! Check your portfolio.`
- `/deletecronalert <id>` - Delete a cron alert by ID
  - Example: `/deletecronalert 1`

### How It Works

#### Price Alerts
1. **Create an Alert**: Use `/setalert` command to set a target price for any supported cryptocurrency
2. **Real-time Monitoring**: The bot continuously monitors prices via WebSocket connections
3. **Alert Triggering**: When the current price reaches your target (within 0.1% tolerance), you'll receive a notification
4. **Cooldown Period**: After triggering, alerts enter a 1-minute cooldown to prevent spam
5. **Auto-reset**: Cooldowns are automatically reset every 5 seconds for future triggers

#### Cron Alerts
1. **Create a Cron Alert**: Use `/setcronalert` command to schedule a daily alert at 8am
2. **Scheduled Execution**: The cron worker checks every minute and triggers alerts at the specified time
3. **Daily Notifications**: You'll receive your scheduled message every day at 8am
4. **Management**: Use `/cronalerts` to view all your scheduled alerts and `/deletecronalert` to remove them

## Configuration

### Supported Cryptocurrencies

The bot supports all SPOT tokens listed on the Hyperliquid exchange. When setting an alert, use the coin's symbol (e.g., `BTC`, `ETH`, `SOL`).

### Price Tolerance

Alerts trigger when the current price is within 0.1% of your target price:
- Lower bound: `current_price * 0.999`
- Upper bound: `current_price * 1.001`

## TODO
- [x] Set alerts via telegram
- [x] Receive notifications via telegram
- [x] Subscribe to multiple tokens
- [x] Isolate alerts to chat ids
- [x] Cron alerts
- [ ] Isolate alerts to addresses  
- [ ] Delete alerts
    - [ ] remove subscriptions if token is no longer monitored
- [ ] Smart alerts
  - [ ] Allow people to submit their public address to auto generate alerts based on their perps positions e.g. if they are within 10% range of being liquidated or their SL/TP prices
- [ ] Measure performance
- [ ] Add a message queue (totally unnecessary for the current scale but should be a fun task)
- [ ] Advanced cron scheduling (hourly, weekly, custom schedules)