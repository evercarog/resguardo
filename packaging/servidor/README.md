# Imagen Docker de Resguardo Server

- `Dockerfile`: imagen (compila con `--locked`, se ejecuta como usuario sin privilegios, datos en el volumen `/data`).

```sh
docker build -f packaging/servidor/Dockerfile -t resguardo-server .
docker run -d --name resguardo -p 8443:8443 -v resguardo-datos:/data resguardo-server
docker logs resguardo                                  # el código de primer arranque
docker exec resguardo resguardo-server codigo-inicial --datos /data
```

La instalación en Debian, Ubuntu o un CT de Proxmox (servicio de systemd,
`.deb` y `.tar.gz`) está en [../linux/](../linux/README.md); la guía paso a
paso, en [docs/servidor-linux.md](../../docs/servidor-linux.md).
