# minilab — build & e2e. Registry pulls are blocked on this host, so the node
# image is built locally: static nodeagent -> tar -> docker import.
BIN   := minilab
IMAGE := minilab/nodeagent:v1
TOPO  := examples/pair.clab.yml

.PHONY: build image e2e test clean

build:
	go build -o $(BIN) .

# image also (re)builds the CLI so `make image && ./minilab deploy ...` works.
image: build
	mkdir -p build
	CGO_ENABLED=0 go build -ldflags '-s -w' -o build/nodeagent ./cmd/nodeagent
	tar -C build -cf build/rootfs.tar nodeagent
	docker import --change 'ENTRYPOINT ["/nodeagent"]' build/rootfs.tar $(IMAGE)

e2e: image
	./$(BIN) deploy -t $(TOPO)
	./$(BIN) inspect -t $(TOPO)
	docker exec minilab-pair-n1 /nodeagent -probe 10.10.0.2:9000
	./$(BIN) destroy -t $(TOPO)
	test -z "$$(docker ps -aq --filter label=minilab.lab)"
	@echo "e2e: OK"

test:
	go test ./...

clean:
	rm -rf $(BIN) build
