.PHONY: all clean generate move

# Variables for paths
PRIME_DIR = ../ersha-prime/keys
DISPATCH_DIR = ../ersha-dispatch/keys
RPC_EXAMPLE_DIR = ../ersha-rpc/examples/keys

# Default target
all: generate move clean_intermediates

generate:
	@echo "Generating mTLS assets..."

	openssl genrsa -out root_ca.key 4096
	openssl req -x509 -new -nodes -key root_ca.key -sha256 -days 1024 -out root_ca.crt \
		-subj "/CN=MyLocalCA"


	openssl genrsa -out server_raw.key 2048
	openssl req -new -key server_raw.key -out server.csr -subj "/CN=localhost"
	openssl x509 -req -in server.csr -CA root_ca.crt -CAkey root_ca.key -CAcreateserial \
		-out server.crt -days 500 -sha256 \
		-extfile <(printf "subjectAltName=DNS:localhost,IP:127.0.0.1")

	openssl genrsa -out client_raw.key 2048
	openssl req -new -key client_raw.key -out client.csr -subj "/CN=my-client"
	openssl x509 -req -in client.csr -CA root_ca.crt -CAkey root_ca.key -CAcreateserial \
		-out client.crt -days 500 -sha256

	openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt -in server_raw.key -out server.key
	openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt -in client_raw.key -out client.key

move:
	@echo "Moving files to service directories..."
	mkdir -p $(PRIME_DIR)
	cp server.crt server.key root_ca.crt $(PRIME_DIR)
	mkdir -p $(DISPATCH_DIR)
	cp client.crt client.key root_ca.crt $(DISPATCH_DIR)
	mkdir -p $(RPC_EXAMPLE_DIR)
	cp server.crt server.key client.crt client.key root_ca.crt $(RPC_EXAMPLE_DIR)

clean_intermediates:
	@echo "Cleaning up CSRs and raw keys..."
	rm -f *.csr server_raw.key client_raw.key root_ca.srl

# Use this to totally wipe the local folder of ALL generated keys
clean:
	@echo "Deleting all local keys and certs..."
	rm -f *.crt *.key *.csr *.srl
