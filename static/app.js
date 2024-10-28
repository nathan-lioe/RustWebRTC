let localStream;
let peerConnection;
const signalingServerUrl = "ws://127.0.0.1:3030/signaling";
let signalingSocket;
const remoteStreams = {};

document.getElementById("join").addEventListener("click", async () => {
    const room = document.getElementById("room").value;
    if (!room) {
        alert("Please enter a room name");
        return;
    }

    signalingSocket = new WebSocket(signalingServerUrl);
    signalingSocket.onopen = () => {
        signalingSocket.send(JSON.stringify({ type: "join", room }));
    };

    signalingSocket.onmessage = async (message) => {
        const data = JSON.parse(message.data);
        switch (data.type) {
            case "offer":
                await handleOffer(data.offer, data.sender);
                break;
            case "answer":
                await handleAnswer(data.answer);
                break;
            case "candidate":
                await handleCandidate(data.candidate);
                break;
            default:
                break;
        }
    };

    localStream = await navigator.mediaDevices.getUserMedia({ video: true, audio: true });
    addVideoStream("localVideo", localStream, true);

    // Enable control buttons
    document.getElementById("start").disabled = false;
    document.getElementById("stop").disabled = false;
});

document.getElementById("start").addEventListener("click", () => {
    startSession();
});

document.getElementById("stop").addEventListener("click", () => {
    stopSession();
});

async function startSession() {
    if (!peerConnection) {
        peerConnection = createPeerConnection();
    }
    const offer = await peerConnection.createOffer();
    await peerConnection.setLocalDescription(offer);
    signalingSocket.send(JSON.stringify({ type: "offer", offer }));
}

function stopSession() {
    if (peerConnection) {
        peerConnection.close();
        peerConnection = null;
    }
    localStream.getTracks().forEach(track => track.stop());
    clearVideoStreams();
    document.getElementById("start").disabled = true;
    document.getElementById("stop").disabled = true;
}

async function handleOffer(offer, sender) {
    peerConnection = createPeerConnection();
    await peerConnection.setRemoteDescription(new RTCSessionDescription(offer));
    const answer = await peerConnection.createAnswer();
    await peerConnection.setLocalDescription(answer);
    signalingSocket.send(JSON.stringify({ type: "answer", answer }));
}

function createPeerConnection() {
    const pc = new RTCPeerConnection();
    pc.onicecandidate = (event) => {
        if (event.candidate) {
            signalingSocket.send(JSON.stringify({ type: "candidate", candidate: event.candidate }));
        }
    };
    pc.ontrack = (event) => {
        const stream = event.streams[0];
        const streamId = event.track.id;
        if (!remoteStreams[streamId]) {
            remoteStreams[streamId] = stream;
            addVideoStream(streamId, stream);
        }
    };
    localStream.getTracks().forEach(track => pc.addTrack(track, localStream));
    return pc;
}

async function handleAnswer(answer) {
    await peerConnection.setRemoteDescription(new RTCSessionDescription(answer));
}

async function handleCandidate(candidate) {
    await peerConnection.addIceCandidate(new RTCIceCandidate(candidate));
}

function addVideoStream(id, stream, isLocal = false) {
    const videoContainer = document.getElementById("videos");
    const videoElement = document.createElement("video");
    videoElement.id = id;
    videoElement.srcObject = stream;
    videoElement.autoplay = true;
    if (isLocal) {
        videoElement.muted = true;
    }
    videoContainer.appendChild(videoElement);
}

function clearVideoStreams() {
    const videoContainer = document.getElementById("videos");
    videoContainer.innerHTML = "";
    Object.keys(remoteStreams).forEach(streamId => {
        remoteStreams[streamId].getTracks().forEach(track => track.stop());
    });
    remoteStreams = {};
}