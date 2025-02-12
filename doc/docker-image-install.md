## Using Docker Pull

Pull the latest image from Github Container Registry:

```sh
docker pull ghcr.io/chrivers/bifrost:master-2025-02-04
```

Curl and rename the example configuration file:

```sh
curl -O https://raw.githubusercontent.com/chrivers/bifrost/master/config.example.yaml
cp config.example.yaml config.yaml
```

And edit it with your favorite editor to your liking (see
[configuration reference](doc/config-reference.md)).

Now run the Docker Container:

```sh
docker run -v $(pwd)/config.yaml:/app/config.yaml ghcr.io/chrivers/bifrost:master-2025-02-04
```

To view the logs, use a tool like [Portainer](https://www.portainer.io/) or
run the following command:

```sh
docker logs bifrost
```
