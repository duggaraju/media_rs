function joinLive() {
    const url = getPlaybackUrl(false);
    if (Hls.isSupported()) {
        window.player.loadSource(url);
    } else {
        window.video.src = url;
    }
    window.video.play();
    console.log("playback started ...");
}

function initializePlayer(video) {
    // Get the player instance from the UI.
    const hls = new Hls({
        renderTextTracksNatively: true,
        debug: true,
        enableWorker: true,
        lowLatencyMode: true,
        streaming: true,
        fetchSetup: (context, init) => {
            const url = new URL(context.url);
            url.search = document.getElementById('token')?.value;
            return new Request(url.toString(), init);
        }
    });
    hls.subtitleDisplay = true;
    // bind them together
    hls.attachMedia(video);
    hls.on(Hls.Events.MEDIA_ATTACHED, (event, data) => {
        console.log(`media attached  ${hls.subtitleTracks.length}`);
    });

    hls.on(Hls.Events.SUBTITLE_TRACKS_UPDATED, (event, data) => {
        console.log(`subtitles updated  ${data.subtitleTracks.length} ${hls.subtitleTrack} ${video.textTracks[0].mode}`);
        hls.subtitleTrack = 0;
    });

    window.player = hls; // for debugging.
}

function getBaseUrl() {
    return baseurl = document.getElementById('url').value;
}

function getPlaybackUrl(useRange) {
    return getBaseUrl();
}

async function uploadVideo()
{
    document.getElementById('url').value = '';
    const video = document.getElementById('file').files[0];
    try {
        const response = await fetch(`/upload/${video.name}`, {method: "POST", body: video});
        const url = await response.text();
        document.getElementById('url').value = new URL(url, document.baseURI);
    }
    catch (e){
        console.error(e);
    }
}

function initialize() {
    var video = document.getElementById('camera');
    window.video = video;
    if (Hls.isSupported()) {
        initializePlayer(video);
    }

    var submit = document.getElementById("submit");
    submit.addEventListener('click', async () => {
        submit.disabled = true;
        submit.innerHTML = "Uploading...";
        await uploadVideo();
        submit.innerHTML = "Upload";
        submit.disabled = false;
    });
}

//if using shaka UI. use this callback instead.
document.addEventListener('DOMContentLoaded', initialize);

