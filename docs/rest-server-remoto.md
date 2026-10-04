# Copias de equipos remotos a tu rest-server (sin abrir puertos)

Esta guía permite que equipos de **otras redes** (clientes, sucursales, portátiles)
guarden sus copias en tu `rest-server` de Proxmox, sin abrir ningún puerto en tu
router y con HTTPS válido, usando **Cloudflare Tunnel** (gratis).

```
Equipo remoto ──HTTPS──▶ Cloudflare ──túnel saliente──▶ cloudflared (CT) ──▶ rest-server :8000
```

El túnel lo abre tu servidor hacia Cloudflare (conexión **saliente**), así que no
hay nada expuesto directamente en tu red.

## Requisitos

- Un dominio gestionado en Cloudflare (p. ej. `tudominio.com`).
- Tu contenedor con `rest-server` (el que ya tienes).

## 1. rest-server seguro para equipos remotos

Cada equipo remoto debe tener **su propio usuario** y **solo poder añadir copias a
su propio repositorio**. Para eso, el rest-server que se publica debe tener:

| Opción | Para qué |
|---|---|
| `--private-repos` | Cada usuario solo ve `/<usuario>/…` |
| `--append-only` | Nadie puede borrar ni modificar copias existentes (protección contra ransomware) |
| `--htpasswd-file` | Usuarios y contraseñas (una por equipo) |

Crear un usuario por equipo (en el CT):

```bash
htpasswd -B /ruta/.htpasswd cliente-altamar
```

> Usa minúsculas en los nombres de usuario (rest-server distingue mayúsculas).

La **retención** de estos repositorios se hace en el CT, sobre la ruta local (no a
través del rest-server), igual que con tus repositorios actuales.

## 2. Instalar cloudflared en el CT

```bash
curl -L https://pkg.cloudflare.com/cloudflare-main.gpg | tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null
echo 'deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main' \
  > /etc/apt/sources.list.d/cloudflared.list
apt update && apt install -y cloudflared
```

## 3. Crear el túnel

```bash
cloudflared tunnel login                 # abre un enlace para autorizar tu dominio
cloudflared tunnel create copias          # crea el túnel y sus credenciales
cloudflared tunnel route dns copias copias.tudominio.com
```

`/etc/cloudflared/config.yml`:

```yaml
tunnel: copias
credentials-file: /root/.cloudflared/<ID-DEL-TUNEL>.json
ingress:
  - hostname: copias.tudominio.com
    service: http://localhost:8000     # el rest-server para equipos remotos
  - service: http_status:404
```

```bash
cloudflared service install
systemctl enable --now cloudflared
```

## 4. Límite de tamaño de Cloudflare

Cloudflare limita el tamaño de cada petición (100 MB en el plan gratuito). restic
sube paquetes de 16 MB por defecto, así que funciona sin cambios. No subas
`--pack-size` por encima de 64 en los equipos remotos.

## 5. En Resguardo (equipo remoto)

Añade un destino **Servidor REST**:

- Ubicación: `rest:https://copias.tudominio.com/cliente-altamar/`
- Usuario: `cliente-altamar` · Contraseña: la de `htpasswd`
- Contraseña del destino: una **distinta para cada equipo** (quien tiene la
  contraseña de un repositorio puede leer todo lo que contiene).

## Seguridad: qué protege cada capa

- **Cloudflare Tunnel**: sin puertos abiertos; HTTPS con certificado válido.
- **Usuario por equipo + `--private-repos`**: un equipo no puede ver los repositorios de otro.
- **`--append-only`**: aunque un equipo remoto sea atacado, no puede borrar sus copias anteriores.
- **Contraseña de restic por equipo**: los datos van cifrados de extremo a extremo;
  ni Cloudflare ni tu servidor pueden leerlos.

Opcional y recomendable: en Cloudflare **Zero Trust → Access**, añade una regla
de *service token* para `copias.tudominio.com`, así solo clientes con ese token
pueden siquiera llegar al rest-server.
