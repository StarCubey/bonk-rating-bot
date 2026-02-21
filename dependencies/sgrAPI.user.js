// ==UserScript==
// @name        sgrAPI 2
// @namespace   Violentmonkey Scripts
// @match       https://bonk.io/*
// @run-at      document-start
// @grant       none
// @version     2.0
// @author      StarCubey
// @license     MIT
// @description sgrAPI with less functionality bundled with injector.
// ==/UserScript==

// Some of the regexes and variable names are copied from bonk host: https://github.com/Salama/bonk-host

/*
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

window.sgrAPI.state.scores and window.sgrAPI.footballState.scores
For team games, the score array will have 4 numbers corresponding to different team colors.
For non-team games, the score array will be similar to the player array where scores[playerId] gives you the score of that player.

window.sgrAPI.nextScores
Setting this variable to a score array will load that score at the beginning of the next game.
This is similar to the Bonk Host keep scores feature. window.sgrBotAPI.nextScores will be undefined when not in use.

sgrAPI.stateFunctions.hostHandlePlayerJoined(id, sgrAPI.players.length, team)
This is bonk host freejoin. This moves a player into a game so that they appear in the next round.
*/

if(window.location.toString() !== "https://bonk.io/sgr") return;

let resolveFunctionsLoaded = () => {};
window.sgrAPIFunctionsLoaded = new Promise(res => resolveFunctionsLoaded = () => res());

let resolveJQueryLoaded = () => {};
window.sgrAPIJQueryLoaded = new Promise(res => resolveJQueryLoaded = () => res());

window.sgrMods = {};

window.sgrMods["sgrAPI"] = code => {
  // Example of input object: { left: false, right: false, up: false, down: false, action: false, action2: false }
  code = `
    window.sgrAPI = {};
    window.sgrAPI.onTick = () => {};
    window.sgrAPI.onInput = input => input;
    window.sgrAPI.bonkCallbacks = {};
    let token = null;
    window.sgrAPI.getToken = () => token;
    ${code}
    `;

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

  //Input Handler
  //Credit to gmmaker for regex: https://github.com/SneezingCactus/gmmaker
  //Original regex: /Date.{0,100}new ([^\(]+).{0,100}\$\(document/
  code = code.replace(
    /Date.{0,100}[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]\[[0-9]{1,3}\];[A-Za-z0-9\$_]{3}\[[0-9]{1,3}\]=new [^\(]+\(\);/,
    match => {
      let inputFunc = match.match(/([A-Za-z0-9\$_]{3}\[[0-9]{1,3}\])=new ([^\(]+)\(\);/);
      return match.replace(inputFunc[0], `${inputFunc[0]} window.sgrAPI.input = ${inputFunc[1]};` +
          `window.sgrAPI.oldGetInputs = window.sgrAPI.input.getInputs;`+
          `window.sgrAPI.input.getInputs = () => window.sgrAPI.onInput(window.sgrAPI.oldGetInputs());`
      );
    }
  );

  // Remove round limit
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

  //Function for all callbacks
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
  code = code.replace("{a:0.0};", "{a:0.0};window.sgrAPI.stateFunctions = this;");
  code = code.replace(
    "newbonklobby_votewindow_close",
    "window.sgrAPI.players = arguments[1]; newbonklobby_votewindow_close",
  );

  // Token
  code = code.replace("[1,10000,25000,100000,500000,8000000,5000000000];", "[1,10000,25000,100000,500000,8000000,5000000000];token = arguments[0];");

  return code;
};

(async () => {
  const frame = await window.gameFrameLoaded;
  const fwin = frame.contentWindow;
  const fdoc = frame.contentDocument;
  window.sgrAPI = fwin.sgrAPI;

  fwin.sgrAPI.startGame = () => {
    for(let callback of Object.keys(fwin.sgrAPI.bonkCallbacks)) {
        fwin.sgrAPI.bonkCallbacks[callback]("startGame");
    }
  };

  // Returns a condensed version of window.sgrBotAPI.players with no null values where each player has an id property.
  fwin.sgrAPI.getPlayers = () => Object.keys(fwin.sgrAPI.players)
    .map(i => {
      let player = fwin.sgrAPI.players[i];
      if(player === null) return undefined;
      player.id = Number(i);
      return player;
    }).filter(p => p);

  // Loads a map object.
  fwin.sgrAPI.loadMap = map => {
    let mapContainer = fdoc.getElementById("maploadwindowmapscontainer");
    while(mapContainer.firstChild) {
      mapContainer.firstChild.remove();
    }
    fwin.sgrAPI.mapLoader({maps: [map]});
    mapContainer.firstChild.click();
  }

  // Football = "f", Simple = "bs", Death Arrows = "ard", Arrows = "ar", Grapple = "sp", VTOL = "v", and Classic = "b".
  fwin.sgrAPI.setMode = m => {
    if(m === "f") {
      fwin.sgrAPI.gameInfo[2].ga = "f";
      fwin.sgrAPI.gameInfo[2].tea = true;
      fwin.sgrAPI.toolFunctions.networkEngine.sendTeamSettingsChange(window.sgrAPI.gameInfo[2].tea);
    } else {
      fwin.sgrAPI.gameInfo[2].ga = "b";
    }
    fwin.sgrAPI.gameInfo[2].mo = m;
    fwin.sgrAPI.menuFunctions.updatePlayers();
    fwin.sgrAPI.toolFunctions.networkEngine.sendGAMO(window.sgrAPI.gameInfo[2].ga, window.sgrAPI.gameInfo[2].mo);
    fwin.sgrAPI.menuFunctions.updateGameSettings();
  };

  // Sets teams to true or false.
  fwin.sgrAPI.setTeams = teams => {
    if(fwin.sgrAPI.gameInfo[2].ga === "f") return;

    fwin.sgrAPI.gameInfo[2].tea = teams;
    fwin.sgrAPI.toolFunctions.networkEngine.sendTeamSettingsChange(teams);
    fwin.sgrAPI.menuFunctions.updatePlayers();
    fwin.sgrAPI.menuFunctions.updateGameSettings();
  };

  // Gets a map response object containing favorited maps. response.maps is an array of map objects.
  // An offset of 0 gives you the first 32 maps and incrementing by 1 gives you the next 32.
  fwin.sgrAPI.getFav = async offset => {
    let response;

    await fwin.$.post("https://bonk2.io/scripts/map_getfave.php", {
      token: sgrAPI.getToken(),
      startingfrom: offset * 32
      }).done(e => {

      if (e.r != "success") console.log("Failed to load favorited maps.");
      else response = e;
    });

    return response;
  }

  // Gets map array from playlist string (from Salama's playlist mod) assuming maps are favorited.
  // https://greasyfork.org/en/scripts/439123-bonk-playlists
  fwin.sgrAPI.fromPlaylist = async (playlist) => {
    playlist = JSON.parse(playlist);
    let bonk2MapIds = playlist.map(p => p.maps).flat();
    let bonk1Maps = playlist.map(p => p.b1maps).flat();

    let foundMaps = [];
    for(i = 0; foundMaps.length < bonk2MapIds.length; i++) {
      let maps = (await sgrAPI.getFav(i)).maps;
      if(!maps || maps.length === 0) break;
      for(map of maps) {
        if(bonk2MapIds.find(id => id === map.id) !== undefined) {
          foundMaps.push(map);
        }
      }
    }

    return [foundMaps, bonk1Maps].flat();
  };

  fwin.sgrAPI.logIn = async (username, password) => {
    fdoc.getElementById("loginwindow_username").value = username;
    fdoc.getElementById("loginwindow_password").value = password;
    while(true) {
      fdoc.getElementById("loginwindow_submitbutton").click();
      await new Promise(result => setTimeout(result, 2000));
      if(fdoc.getElementById("classic_mid").style.visibility === "inherit") break;
    }
    fdoc.getElementById("guestOrAccountContainer").style.visibility = "hidden";
  }

  //Makes a room and returns the room link.
  fwin.sgrAPI.makeRoom = async (name, password, maxPlayers, minLevel, maxLevel, unlisted) => {
    while(true) {
      fdoc.getElementById("roomlistrefreshbutton").click();
      fdoc.getElementById("roomlistcreatewindowgamename").value = name;
      fdoc.getElementById("roomlistcreatewindowpassword").value = password;
      fdoc.getElementById("roomlistcreatewindowmaxplayers").value = maxPlayers;
      fdoc.getElementById("roomlistcreatewindowminlevel").value = minLevel;
      fdoc.getElementById("roomlistcreatewindowmaxlevel").value = maxLevel;
      fdoc.getElementById("roomlistcreatewindowunlistedcheckbox").checked = unlisted;
      fdoc.getElementById("roomlistcreatecreatebutton").click();
      await new Promise(result => setTimeout(result, 2000));
      let connectStr = fdoc.getElementById("sm_connectingWindow_text").innerText;
      let connectVisibility = fdoc.getElementById("sm_connectingContainer").style.visibility;
      if(connectStr !== "Creating room...\nConnect error" && connectVisibility !== "hidden" && connectVisibility !== "") {
        while(true) {
          fdoc.getElementById("newbonklobby_linkbutton").click();
          await new Promise(result => setTimeout(result, 500));
          let messages = fdoc.getElementById("newbonklobby_chat_content").children;
          if(messages.length > 2) {
            return `https://bonk.io/${messages[messages.length-1].innerText.match(/\/([a-z0-9]+)/)[1]}`;
          }
        }
      }
    }
  }

  //Listens to incoming WebSocket messages. Return true if you want to continue with default behavior.
  fwin.sgrAPI.onReceive = message => true;
  //Listens to outgoing WebSocket messages. Return true if you want to continue with default behavior.
  fwin.sgrAPI.onSend = message => true;

  //Sends a WebSocket message.
  fwin.sgrAPI.send = message => {
    fwin.sgrAPI.socket.send(message);
  };

  fwin.sgrAPI.originalSend = fwin.WebSocket.prototype.send;
  fwin.WebSocket.prototype.send = function(args) {
    let sendFilter;

    if (this.url.includes("socket.io/?EIO=3&transport=websocket&sid=") && !this.noSpoof) {
      if (!this.injectedAPI) {
        fwin.sgrAPI.socket = this;
        this.injectedAPI = true;

        fwin.sgrAPI.originalReceive = this.onmessage;
        this.onmessage = function(args) {
          let receiveFilter = fwin.sgrAPI.onReceive(args.data);
          if(receiveFilter === undefined || receiveFilter === true) return fwin.sgrAPI.originalReceive.call(this, args);
          else return;
        }
      } else {
        sendFilter = fwin.sgrAPI.onSend(args);
      }
    }

    if(sendFilter === undefined || sendFilter === true) return fwin.sgrAPI.originalSend.call(this, args);
    else return;
  }

  fwin.sgrAPI.oldPost = () => {};

  //This function can be overwritten to spoof or get data from post requests.
  fwin.sgrAPI.onPost = (url, input) => fwin.sgrAPI.oldPost(url, input).then((output, status) => {
    return output;
  });

  fdoc.getElementById("newbonklobby_roundsinput").addEventListener("focus", e => {
  e.target.value = "";
  });
  fdoc.getElementById("newbonklobby_roundsinput").addEventListener("blur", e => {
    if(e.target.value == "") {
      e.target.value = fwin.sgrAPI.toolFunctions.getGameSettings().wl;
    }
  });

  //This function is used with onPost for wrapping post responses in JQuery promises.
  fwin.sgrAPI.postResponse = value => {
    const deferred = fwin.$.Deferred();
    deferred.resolve(value, 'success', { status: 200 });
    return deferred.promise();
  }

  let jQueryInterval = setInterval(() => {
    if(fwin.$) {
      fwin.sgrAPI.oldPost = fwin.$.post;
      fwin.$.post = function(url, input) {
        const output = fwin.sgrAPI.onPost(url, input);
        if(output === undefined) return fwin.sgrAPI.oldPost(...arguments);

        return output;
      }

      clearInterval(jQueryInterval);
      resolveJQueryLoaded();
    }
  }, 250);

  resolveFunctionsLoaded();
})();

/*
// Favorites a map.
window.sgrAPI.fav = async id => {
  await $.post("https://bonk2.io/scripts/map_fave.php", {
	  token: sgrAPI.token,
		mapid: id,
		action: "f"
		}).fail(e => {

    console.log("Failed to favorite map:" + e);
  });
};

//onPost example. Changes username on login. Fake username is only visible for you.
sgrAPI.onPost = (url, input) => sgrAPI.oldPost(url, input).then((output, status) => {
  if(url.endsWith("login_auto.php") || url.endsWith("login_legacy.php")) {
    output.username = "Fake username";
  }

  return output;
});

//Example of sending spoofed data for a post response without retrieving data from bonk servers.
sgrAPI.onPost = (url, input) => {
  if(url.endsWith("map_getfave.php")) {
      return postResponse({
        maps: [{
          "id": 123,
          "name": "Simple 1v1",
          "authorname": "GudStrat",
          "leveldata": "ILDuJAhZIawhiQEVgGkCqAmANgFwGMBxADxwEkARAMSwFlVyAlAZgDVYMWmBPATQAaqGAEsArhACiAVlTRgACwASAEwDqTACoqlAKQUqkwAKahJCAMIgAHCgTngGSKGGJklV0a-efSByAA5NjVpMT41AEYcAC0LSE0AQyJqUGiBAHoAN3Sc3JyodIB2PLyWLJLc4CIAWwA2FQBzIzkCcDo6AT4ScmpIAGdBJmqAIxZdPF9J7wAFAGofbO90gAYALzp1zbpwKd29-YPJh2RJaBP5f2ALUGp4JEpgAHlPQ69Ic+BPNk1iahZh2CQaJeUCUO6vHxOT6XahcSAGLAAFkQQA",
          "publisheddate": "2020-05-05 16:59:52",
          "vu": 71626,
          "vd": 14821,
          "remixname": "",
          "remixauthor": "",
          "remixdb": 1,
          "remixid": 0,
        }],
        more: false,
        r: "success",
      });
  };
};
*/
