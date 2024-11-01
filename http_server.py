import http.server
import ssl

# Define the IP address and port to serve on
IP_ADDRESS = "YOURADRESS"  # Change to "0.0.0.0" if you want to access from other devices
PORT = 8000

# Set up a simple HTTP request handler
handler = http.server.SimpleHTTPRequestHandler

# Create an HTTP server instance
httpd = http.server.HTTPServer((IP_ADDRESS, PORT), handler)

# Set up SSL context
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(certfile="cert.pem", keyfile="key.pem")

# Wrap the HTTP server socket with SSL
httpd.socket = context.wrap_socket(httpd.socket, server_side=True)

print(f"Serving on https://{IP_ADDRESS}:{PORT}")
httpd.serve_forever()
