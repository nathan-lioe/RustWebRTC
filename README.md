## WebRTC with Rust


### **Start by cloning the repo to your local machine**
````
git clone https://github.com/nathan-lioe/RustWebRTC
cd RustWebRTC
````

### **Run signaling server:**

````
cargo run
````


- This command compiles and runs your Rust application, starting the signaling server on http://127.0.0.1:3030/signaling



### **Navigate to the directory containing your index.html file and run:**
````
cd static
python3 -m http.server 8000
````

- This command starts a server at http://localhost:8000

--- 
### **Yay you're almost there!!**

Open the application in a browser via:

```` 
http://localhost:8000
````


### Testing the Application:

**Join a Room:**
  - Enter a room name in the input field and click the "Join" button.
  - This should connect you to the signaling server and enable the "Start" and "Stop" buttons.
    
**Start the Session:**
  - Click the "Start" button to initiate the WebRTC session.
  - Your local video stream should appear on the screen.
    
**Open Another Browser Window:**
  - Open another browser window or tab and navigate to http://localhost:8000.
  - Join the same room name as the first participant.
  - Both participants should see each other's video streams.
    
**Stop the Session:**
  - Click the "Stop" button to end the WebRTC session.
  - This should stop the video streams and clear the video elements.


