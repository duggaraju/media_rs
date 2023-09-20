async function load(url) {
    try {
        await player.load(url);
    } catch(err) {
        console.log(err);
        throw err;
    }
    // This runs if the asynchronous load is successful.
    console.log("playback started ...");
}

async function play() {
    const url = getPlaybackUrl(false);
    await load(url);
    video.play();
}

function initializeShakaPlayer(video) {
    // Get the player instance from the UI.
    const ui = video['ui'];
    const controls = ui.getControls();
    const player = controls.getPlayer();
    player.configure({
      streaming: {
        bufferingGoal: 1,
        //rebufferingGoal: 0.5,
        autoLowLatencyMode: true,
        useNativeHlsOnSafari: true,
        alwaysStreamText: true
      },
      manifest: {
        defaultPresentationDelay: 0.1,
        availabilityWindowOverride: 30,
      }
    });

    window.player = player; // for debugging.
    player.addEventListener('error', event => {
        console.log(`error playing ${event.detail}`);
    });    
}

function getPlaybackUrl(useRange) {
    return document.getElementById('url').value;
}

function initialize() {
    var video = document.getElementById('video');
    window.video = video;
    initializeShakaPlayer(video);
}

shaka.log.setLevel(shaka.log.Level.DEBUG);
shaka.polyfill.installAll();
 
//if using shaka UI. use this callback instead.
document.addEventListener('shaka-ui-loaded', initialize);
