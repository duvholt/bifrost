## Using Docker Pull

Pull the latest image from Github Container Registry:

```sh
docker pull ghcr.io/duvholt/bifrost:latest
```

Curl and rename the example configuration file:

```sh
curl -O https://raw.githubusercontent.com/duvholt/bifrost/master/config.example.yaml
cp config.example.yaml config.yaml
```

And edit it with your favorite editor to your liking (see
[configuration reference](config-reference.md)).

Now run the Docker Container:

```sh
docker run -v $(pwd)/config.yaml:/app/config.yaml ghcr.io/duvholt/bifrost:latest
```

To view the logs, run the following command:

```sh
docker logs bifrost
```
