# Bonk MMR

This is a bot that automatically hosts rooms and calculates ratings using a statistical rating algorithm.

# Manually Installed Dependencies

## WebDriver

The reccomended option is Chrome.

You can install Chrome for testing here: https://googlechromelabs.github.io/chrome-for-testing/

You will need "chrome" or "chrome-headless-shell" as well as "chromedriver". The path for the chrome binary must added as an environment variable or included in .env. When running the bot, chromedriver must be running in the background with the command "chromedriver --port==9515" with the port being whatever you specify in the environment variable.

## Bonk.io Code Injector

Currently, the only option is Excigma and kklkkj's code injector availible here: https://greasyfork.org/en/scripts/433861-code-injector-bonk-io

The code injector must be placed in the "dependencies" folder under the name "Injector.js".

## Discord Bot

You will need to make a discord bot and add the token as an environment variable.

# Distributed Dependencies

These mods are included in the dependencies folder for convenience.

(Bonk Host)[https://greasyfork.org/en/scripts/435169-bonk-host] by Salama_

(Bonk Playlists)[https://greasyfork.org/en/scripts/439123-bonk-playlists] by Salama_

# License

The entire project except for the contents of the dependencies folder is licensed under MIT.
