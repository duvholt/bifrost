## Using Docker Pull

Pull the latest image from Github Container Registry:

```sh
docker pull ghcr.io/chrivers/bifrost:latest
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
docker run -v $(pwd)/config.yaml:/app/config.yaml ghcr.io/chrivers/bifrost:latest
```

To view the logs, run the following command:

```sh
docker logs bifrost
```
