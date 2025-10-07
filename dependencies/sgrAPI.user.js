// ==UserScript==
// @name        bonk.io sgrAPI
// @namespace   Violentmonkey Scripts
// @match       https://bonk.io/gameframe-release.html
// @run-at      document-start
// @grant       none
// @version     1.0
// @author      StarCubey
// @license     MIT
// @description An API for bots or doing various actions programatically.
// ==/UserScript==

/*
This mod requires Excigma's code injector to be installed
https://greasyfork.org/en/scripts/433861-code-injector-bonk-io

This mod is not compatible with Salama's bonk host because you can't inject Salama's football step regex twice.

All regexes and most reverse engineering logic is copied from "bonk-host" and "bonk-playlists" by Salama.
Otherwise, the code is written by me and licensed under MIT.
https://github.com/Salama/bonk-host
https://github.com/Salama/bonk-playlists

Copyright (c) 2021 Salama

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

You can access the window.sgrAPI object from inspect element. Just make sure you're in the correct frame which can be done by
right clicking the game and selecting inspect element or by selecting the frame gameframe-release.html.

window.sgrAPI.players
This is an array of players where the index is the player id and the value has player specific information.
The player array may have a lot of empty spaces (with a value of null) since each new player is given a new id.

window.sgrAPI.toolFunctions.networkEngine
Contains functions for common actions like moving players, changing host, or kicking players.
Many of these functions take a player id as a parameter.

networkEngine functions and variables:
getLSID() - Gets your player id
hostID - Variable that contains the id of the host
chatMessage("message") - Sends a chat message
kickPlayer(id)
banPlayer(id)
changeOwnTeam(team) - Spectate = 0, Playing = 1, Red = 2, Blue = 3, Green = 4, Yellow = 5
changeOtherTeam(id, team)
doTeamLock(true/false) - locks or unlocks teams
sendNoHostSwap(true/false) - If set to false, host does not swap on disconnect.
setReady(true/false) - Changes your ready state.
allReadyReset() - Sets the ready state of all players.
sendStartCountdown(num) - Sends "Game starting in num"

window.sgrAPI.state and sgrBotAPI.footballState
The state values have useful information like score and player position.

window.sgrAPI.state.score and window.sgrBotAPI.footballState.score
For team games, the score array will have 4 numbers corresponding to different team colors.
For non-team games, the score array will be similar to the player array where scores[playerId] gives you the score of that player.

window.sgrAPI.nextScores
Setting this variable to a score array will load that score at the beginning of the next game.
This is similar to the Bonk Host keep scores feature. window.sgrBotAPI.nextScores will be undefined when not in use.
*/

window.sgrAPI = {};

window.sgrAPI.startGame = () => {
  for(let callback of Object.keys(window.sgrAPI.bonkCallbacks)) {
    	window.sgrAPI.bonkCallbacks[callback]("startGame");
	}
}

// Returns a condensed version of window.sgrBotAPI.players with no null values where each player has an id property.
window.sgrAPI.getPlayers = () => Object.keys(window.sgrAPI.players)
  .map(i => {
    let player = window.sgrAPI.players[i];
    if(player === null) return undefined;
    player.id = Number(i);
    return player;
  }).filter(p => p);

// Football = "f", Simple = "bs", Death Arrows = "ard", Arrows = "ar", Grapple = "sp", VTOL = "v", and Classic = "b".
window.sgrAPI.setMode = m => {
  if(m === "f") {
    window.sgrAPI.gameInfo[2].ga = "f";
    window.sgrAPI.gameInfo[2].tea = true;
		window.sgrAPI.toolFunctions.networkEngine.sendTeamSettingsChange(window.sgrAPI.gameInfo[2].tea);
  } else {
    window.sgrAPI.gameInfo[2].ga = "b";
  }
  window.sgrAPI.gameInfo[2].mo = m;
  window.sgrAPI.menuFunctions.updatePlayers();
  window.sgrAPI.toolFunctions.networkEngine.sendGAMO(window.sgrAPI.gameInfo[2].ga, window.sgrAPI.gameInfo[2].mo);
  window.sgrAPI.menuFunctions.updateGameSettings();
};

// Sets teams to true or false.
window.sgrAPI.setTeams = teams => {
  if(window.sgrAPI.gameInfo[2].ga === "f") return;

  window.sgrAPI.gameInfo[2].tea = teams;
  window.sgrAPI.toolFunctions.networkEngine.sendTeamSettingsChange(teams);
  window.sgrAPI.menuFunctions.updatePlayers();
  window.sgrAPI.menuFunctions.updateGameSettings();
};

// Loads a map object.
window.sgrAPI.loadMap = map => {
  let mapContainer = document.getElementById("maploadwindowmapscontainer");
  while(mapContainer.firstChild) {
    mapContainer.firstChild.remove();
  }
  window.sgrAPI.mapLoader({maps: [map]});
  mapContainer.firstChild.click();
}

// Gets a map response object containing favorited maps. response.maps is an array of map objects.
// An offset of 0 gives you the first 32 maps and incrementing by 1 gives you the next 32.
window.sgrAPI.getFav = async offset => {
  let response;

  await $.post("https://bonk2.io/scripts/map_getfave.php", {
    token: token,
    startingfrom: offset * 32
    }).done(e => {

    if (e.r != "success") console.log("Failed to load favorited maps.");
    else response = e;
  });

  return response;
}

// Favorites a map.
window.sgrAPI.fav = async id => {
  await $.post("https://bonk2.io/scripts/map_fave.php", {
	  token: token,
		mapid: id,
		action: "f"
		}).fail(e => {

    console.log("Failed to favorite map:" + e);
  });
};

// Left = 37, Up = 38, Right = 39, Down = 40, X = 88, Z = 90
// https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/keyCode
window.sgrAPI.keyDown = keyCode => {
  let event = document.createEvent("HTMLEvents")
  event.initEvent("keydown", true, false);
  event["keyCode"] = keyCode;
  document.getElementById("gamerenderer").dispatchEvent(event);
};

window.sgrAPI.keyUp = keyCode => {
  let event = document.createEvent("HTMLEvents")
  event.initEvent("keyup", true, false);
  event["keyCode"] = keyCode;
  document.getElementById("gamerenderer").dispatchEvent(event);
};

// This function is called every game tick.
window.sgrAPI.onTick = () => {}

// This function is called every time a websocket message is received.
// More information here: https://github.com/UnmatchedBracket/DemystifyBonk/blob/main/Packets.md
// message is a string. For instance, chat messages always follow the format of 42[20,playerId,"message"].
window.sgrAPI.onReceive = message => {}

window.sgrAPI.send = message => {
  window.sgrAPI.socket.send(message);
};

if(!window.bonkCodeInjectors) window.bonkCodeInjectors = [];
window.bonkCodeInjectors.push((code) => {
  // Step functions
  code = code.replace(
    /[A-Za-z]\[[A-Za-z0-9\$_]{3}(\[[0-9]{1,3}\]){2}\]={discs/,
    match => "window.sgrAPI.state = arguments[0]; window.sgrAPI.onTick();" + match,
  );
  code = code.replace(
    /=\[\];if\(\![A-Za-z0-9\$_]\[[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]\[[0-9]{1,3}\]\]\)\{/,
    match => {
      match = match.split(";");
      return match[0] + ";window.sgrAPI.footballState = arguments[0]; window.sgrAPI.onTick();" + match[1];
    },
  );

  // Set state
  let stateCreationString = code.match(/[A-Za-z]\[...(\[[0-9]{1,4}\]){2}\]\(\[\{/)[0];
  let stateCreationStringIndex = stateCreationString.match(/[0-9]{1,4}/g);
  stateCreationStringIndex = stateCreationStringIndex[stateCreationStringIndex.length - 1];
  let stateCreation = code.match(`[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]=[A-Za-z0-9\$_]{3}\\[[0-9]{1,4}\\]\\[[A-Za-z0-9\$_]{3}\\[[0-9]{1,4}\\]\\[${stateCreationStringIndex}\\]\\].+?(?=;);`)[0];
  stateCreationString = stateCreation.split(']')[0] + "]";
  code = code.replace(
    /\* 999\),[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\],null,[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\],true\);/,
    match => match + `
      if(window.sgrAPI.nextScores) {
        ${stateCreationString}.scores = sgrAPI.nextScores;
      }
      window.sgrAPI.nextScores = undefined;
      `,
  );

  // Remove round limit
  document.getElementById('newbonklobby_roundsinput').removeAttribute("maxlength");
  code = code.replace(
    /[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]\[[0-9]{1,3}\]\[[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]\[[0-9]{1,3}\]\]=Math\[[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]\[[0-9]{1,3}\]\]\(Math\[[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]\[[0-9]{1,3}\]\]\(1,[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]\[[0-9]{1,3}\]\[[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]\[[0-9]{1,3}\]\]\),9\);/,
    "",
  );
  code = code.replace(
    /[A-Za-z0-9\$_]{3}\[[0-9]{1,4}\]=parseInt\([A-Za-z0-9\$_]{3}(\[0\]){2}\[[A-Za-z0-9\$_]{3}(\[[0-9]{1,4}\]){2}\]\)\;/,
    match => {
      let roundValVar = match.split("=")[0];
      return `${roundValVar}=parseInt(document.getElementById('newbonklobby_roundsinput').value); if(isNaN(${roundValVar}) || ${roundValVar} <= 0) {return;}`;
    },
  );

  // Map loader
  let mapLoader = code.match(/maploadwindowsearchinput.{0,200}else if\([A-Za-z0-9\$_]{3}\[0\]\[0\]\[[A-Za-z0-9\$_]{3}\[[0-9]+\][[0-9]+\]\] == [A-Za-z0-9\$_]{3}\.[A-Za-z0-9\$_]{3}\([0-9]+\)\)\{[A-Za-z0-9\$_]{3}\([A-Za-z0-9\$_]{3}\[0\]\[0\]\);[A-Za-z0-9\$_]{3}\[[0-9]+\]=[A-Za-z0-9\$_]{3}\[0\]\[0\]\[[A-Za-z0-9\$_]{3}\[[0-9]+\]\[[0-9]+\]\];\}\}\)/g)[0].match(/[A-Za-z0-9\$_]{3}\([A-Za-z0-9\$_]{3}\[0\]\[0\]\);/)[0].slice(0, 3);
  code = code.replace(
    `function ${mapLoader}`,
    `window.sgrAPI.mapLoader=${mapLoader};function ${mapLoader}`
  );

  // Token
  code = code.replace(
    "[1,10000,25000,100000,500000,8000000,5000000000];",
    match => match + "window.sgrAPI.setToken(arguments[0]);",
  );

  //Function for all callbacks
  window.sgrAPI.bonkCallbacks = {};
  let callbacks = [...code.match(/[A-Za-z0-9\$_]{3}\(\.\.\./g)];
  for(let callback of callbacks) {
    code = code.replace(
      `function ${callback}`,
      `window.sgrAPI.bonkCallbacks["${callback.split("(")[0]}"] = ${callback.split("(")[0]};` + `function ${callback}`,
    );
  }

  // Useful functions
  code = code.replace(
    /== 13\){...\(\);}}/,
    match => match + "window.sgrAPI.menuFunctions = this;",
  );
  code = code.replace(
    /=new [A-Za-z0-9\$_]{1,3}\(this,[A-Za-z0-9\$_]{1,3}\[0\]\[0\],[A-Za-z0-9\$_]{1,3}\[0\]\[1\]\);/,
    match => match + "window.sgrAPI.toolFunctions = this;",
  );
  code = code.replace(
    /[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]=\{id:-1,element:null\};/,
    match => match + "window.sgrAPI.gameInfo = arguments;",
  );
  code = code.replace(
    "newbonklobby_votewindow_close",
    "window.sgrAPI.players = arguments[1]; newbonklobby_votewindow_close",
  );

  console.log("sgrAPI run");
  return code;
});

let token = null;
window.sgrAPI.setToken = t => {
	token = t;
};

document.getElementById("newbonklobby_roundsinput").addEventListener("focus", e => {
	e.target.value = "";
});
document.getElementById("newbonklobby_roundsinput").addEventListener("blur", e => {
	if(e.target.value == "") {
		e.target.value = window.bonkHost.toolFunctions.getGameSettings().wl;
	}
});

window.sgrAPI.originalSend = window.WebSocket.prototype.send;
window.WebSocket.prototype.send = function(args) {
  if (this.url.includes("socket.io/?EIO=3&transport=websocket&sid=")) {
    if (!this.injectedAPI) {
      window.sgrAPI.socket = this;
      this.injectedAPI = true;

      window.sgrAPI.originalReceive = this.onmessage;
      this.onmessage = function(args) {
        window.sgrAPI.onReceive(args.data);
        return window.sgrAPI.originalReceive.call(this, args);
      }
    }
  }

  return window.sgrAPI.originalSend.call(this, args);
}

console.log("sgrAPI loaded");
