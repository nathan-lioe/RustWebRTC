

const servers = {
  iceServers: [
    {
      urls: ['stun:stun1.l.google.com:19302', 'stun:stun2.l.google.com:19302'],
    },
  ],
  iceCandidatePoolSize: 10,
};

// Global State
const pc = new RTCPeerConnection(servers);
let localStream = null;
let remoteStream = null;
let ws; // WebSocket connection

// HTML elements
const webcamButton = document.getElementById('webcamButton');
const webcamVideo = document.getElementById('webcamVideo');
const callButton = document.getElementById('callButton');
const callInput = document.getElementById('callInput');
const answerButton = document.getElementById('answerButton');
const remoteVideo = document.getElementById('remoteVideo');
const hangupButton = document.getElementById('hangupButton');

// Initialize WebSocket connection
function initWebSocket() {
  ws = new WebSocket('ws://127.0.0.1:3030/signaling');

  ws.onopen = () => {
    console.log('WebSocket connection established.');
  };

  ws.onmessage = (event) => {
    console.log('Message received from server:', event.data);
    handleSignalingMessage(JSON.parse(event.data));
  };

  ws.onerror = (error) => {
    console.error('WebSocket error:', error);
  };

  ws.onclose = () => {
    console.log('WebSocket connection closed.');
  };
}

// Handle signaling messages from server
function handleSignalingMessage(message) {
  switch (message.type) {
    case 'answer':
      console.log('Answer received from remote peer:', message.sdp);
      pc.setRemoteDescription(new RTCSessionDescription(message.sdp));
      break;
    case 'candidate':
      console.log('ICE candidate received from remote peer:', message.candidate);
      pc.addIceCandidate(new RTCIceCandidate(message.candidate));
      break;
    default:
      console.error('Unknown signaling message type:', message.type);
  }
}

// 1. Setup media sources
webcamButton.onclick = async () => {
  localStream = await navigator.mediaDevices.getUserMedia({ video: true, audio: true });
  remoteStream = new MediaStream();

  // Push tracks from local stream to peer connection
  localStream.getTracks().forEach((track) => {
    pc.addTrack(track, localStream);
    console.log('Local track added:', track);
  });

  // Pull tracks from remote stream, add to video stream
  pc.ontrack = (event) => {
    event.streams[0].getTracks().forEach((track) => {
      remoteStream.addTrack(track);
      console.log('Remote track added:', track);
    });
  };

  webcamVideo.srcObject = localStream;
  remoteVideo.srcObject = remoteStream;

  callButton.disabled = false;
  answerButton.disabled = false;
  webcamButton.disabled = true;
};

// 2. Create an offer
callButton.onclick = async () => {
  console.log('Creating offer...');
  pc.onicecandidate = (event) => {
    if (event.candidate) {
      console.log('Sending ICE candidate:', event.candidate);
      ws.send(JSON.stringify({ type: 'candidate', candidate: event.candidate }));
    }
  };

  const offerDescription = await pc.createOffer();
  await pc.setLocalDescription(offerDescription);
  console.log('Offer created:', offerDescription);

  // Send offer to server
  ws.send(JSON.stringify({ type: 'offer', sdp: offerDescription }));

  hangupButton.disabled = false;
};

// 3. Answer the call with the unique ID
answerButton.onclick = async () => {
  const callId = callInput.value;
  console.log('Answering call with ID:', callId);

  pc.onicecandidate = (event) => {
    if (event.candidate) {
      console.log('Sending ICE candidate:', event.candidate);
      ws.send(JSON.stringify({ type: 'candidate', candidate: event.candidate }));
    }
  };

  // In a real application, you'd retrieve the offer from a signaling server
  // Here we are directly setting the remote description from the input call ID
  const offer = { /* Insert the offer you received from the caller */ };
  await pc.setRemoteDescription(new RTCSessionDescription(offer));

  const answerDescription = await pc.createAnswer();
  await pc.setLocalDescription(answerDescription);
  console.log('Answer created:', answerDescription);

  // Send answer to server
  ws.send(JSON.stringify({ type: 'answer', sdp: answerDescription }));
};

// Initialize WebSocket connection on page load
window.onload = () => {
  initWebSocket();
};
