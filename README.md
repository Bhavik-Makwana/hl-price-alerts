# Hyperliquid Price Alert Bot

A Telegram bot that monitors cryptocurrency prices on the Hyperliquid exchange and sends real-time price alerts to users when their target prices are reached.

## Features

- 🔔 **Real-time Price Monitoring**: Continuously monitors cryptocurrency prices using Hyperliquid's WebSocket API
- 📱 **Telegram Integration**: Sends instant notifications via Telegram when price targets are hit
- 💾 **Persistent Storage**: SQLite database to store user alerts and settings
- ⏰ **Cooldown System**: Prevents spam by implementing cooldown periods for triggered alerts
- 🎯 **Multi-token Support**: Monitor multiple cryptocurrencies simultaneously
- 🔄 **Auto-reset**: Automatically resets alert cooldowns for future triggers

## Architecture

The application consists of several key components:

- **AlertService**: Manages alert creation, retrieval, and triggering logic
- **NotificationService**: Handles Telegram bot commands and message sending
- **Database**: SQLite-based storage for alerts and user data
- **WebSocket Client**: Real-time price monitoring via Hyperliquid API


### Telegram Commands

- `/help` - Display available commands
- `/alert` - View all your current alerts
- `/setalert <coin> <price>` - Create a new price alert
  - Example: `/setalert HYPE 100.0`

### How It Works

1. **Create an Alert**: Use `/setalert` command to set a target price for any supported cryptocurrency
2. **Real-time Monitoring**: The bot continuously monitors prices via WebSocket connections
3. **Alert Triggering**: When the current price reaches your target (within 0.1% tolerance), you'll receive a notification
4. **Cooldown Period**: After triggering, alerts enter a 1-minute cooldown to prevent spam
5. **Auto-reset**: Cooldowns are automatically reset every 5 seconds for future triggers

## Configuration

### Supported Cryptocurrencies

The bot supports all SPOT tokens listed on the Hyperliquid exchange. When setting an alert, use the coin's symbol (e.g., `BTC`, `ETH`, `SOL`).

### Price Tolerance

Alerts trigger when the current price is within 0.1% of your target price:
- Lower bound: `current_price * 0.999`
- Upper bound: `current_price * 1.001`

## TODO

[] Delete alerts
    [] remove subscriptions if token is no longer monitored
[] Smart alerts
    [] Allow people to submit their public address to auto generate alerts based on their perps positions e.g. if they are within 10% range of being liquidated or their SL/TP prices
[] Measure performance
[] Add a message queue (totally unncessary for the current scale but should be a fun task)
[] Cron alerts