# Install

Installation is somewhat involved. This is a overview of what needs to be done in order to install the bot. Not everything is described in detail.

## Rust

Obviously, Rust along with any dependencies needed for compilation will need to be installed.

## Chrome Driver

You can install chrome driver here: https://googlechromelabs.github.io/chrome-for-testing/

Download either chrome-headless-shell or chrome as well as the corresponding chromedriver. Add chromedriver to PATH. Then, set the CHROME_PATH environment variable to wherever the chrome or chorme headless binary is.

Set the CHROMEDRIVER_PORT to whatever port you want to run it on. Here's what the command might look like if you run chromedriver on port 9515:

`chromedriver --port=9515`

## Bonk.io Bot Account

This is pretty simple. Just set BONK_USERNAME and BONK_PASSWORD environment variables for the account the bot will use.

## Discord Bot

To create a new discord bot, go to the application page: https://discord.com/developers/applications

Start by creating a new application.
To make the bot private, go to "Installation", set the "Install Link" to None, go to "Bot", and turn off "Public Bot".

Go to "OAuth2" and copy the client id. Then, paste this URL in your browser with "CLIENT_ID" replaced with your client id to add the bot to your server: https://discord.com/oauth2/authorize?client_id=CLIENT_ID&permissions=2048&integration_type=0&scope=bot

Go to "Bot" and generate a token which will need to be stored in the DISCORD_TOKEN environment variable.

Then, you need to set the DISCORD_USER_ID environment variable to your user id to ensure that you have bot admin permissions. You can get your user id by going to User Settings > Advanced under the App Settings heading and enabling Developer Mode. Then you can press Copy User ID from your profile.

The DISCORD_SERVER_LINK is just the bot's response when someone runs !discord. It can be set to a server's permentant invite link.

## PostgreSQL

Start by installing PostgreSQL. on debian based distros, it can be installed like so:

```sh
sudo apt update
sudo apt install postgresql
```

Next, you'll need to set the DATABASE_URL environment variable. With minimal setup, you could just set a password for the postgres user and connect to localhost.

## Environment Varibales

Here's a list of environment variables that need to be set based on the configuration steps desribed above:

```env
BONK_USERNAME=
BONK_PASSWORD=
DISCORD_TOKEN=
DISCORD_USER_ID=
DISCORD_SERVER_LINK=
CHROME_PATH=
CHROMEDRIVER_PORT=
DATABASE_URL=
```
