## Building From Source

When you have these things available, you can install Bifrost by running these commands:

```sh
git clone https://github.com/chrivers/bifrost
cd bifrost
```

Then rename or copy our `config.example.yaml`:

```sh
cp config.example.yaml config.yaml
```

And edit it with your favorite editor to your liking (see
[configuration reference](config-reference.md)).

If you want to put your configuration file or the certificates Bifrost creates somewhere
else, you also need to adjust the mount paths in the `docker-compose.yaml`. Otherwise,
just leave the default values.

Now you are ready to run the app with:

```sh
docker compose up -d
```

This will build and then start the app on your Docker instance.

To view the logs, run the following command:

```sh
docker logs bifrost
```
