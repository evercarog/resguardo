# Publicar una versión del agente (con actualización automática)

Los pasos y las órdenes. El porqué y el modelo de amenazas, en [actualizaciones.md](actualizaciones.md).

> La **llave privada** de publicación solo la tienes tú, fuera de línea. Ningún programa de Resguardo la lee: la usa `minisign`, que pide su contraseña en la terminal. Nunca la copies al repositorio, a un servidor, a la nube ni a un chat.

---

## 1. Una vez: instalar minisign (Windows)

Una de estas tres:

- **winget:** `winget install jedisct1.minisign` (si no lo encuentra: `winget search minisign` y usa el id que salga, del autor `jedisct1`).
- **scoop:** `scoop install minisign`.
- **El zip oficial:** de <https://github.com/jedisct1/minisign/releases>, el `minisign-…-win64.zip` **y** su `.minisig`. Descomprímelo en una carpeta tuya (p. ej. `C:\Herramientas\minisign`) y añádela al `PATH`.

**Compruébalo** antes de usarlo para algo serio:

- El zip: con un minisign que ya tengas (o en Linux, `apt install minisign`), `minisign -Vm minisign-…-win64.zip -P <la llave pública del autor>`. La llave del autor está en el `README` del repositorio de minisign; compárala también con la de su web (<https://jedisct1.github.io/minisign/>). Si no coincide en los dos sitios, no lo uses.
- Con winget o scoop: `Get-FileHash (Get-Command minisign).Source` y compara el SHA-256 con el del zip oficial de esa versión.
- `minisign -v` dice la versión (hace falta la 0.10 o posterior; con una anterior, el script pide `-H`, que ya pasa).

## 2. Una vez: crear la llave (fuera de línea)

En un equipo sin red o, como poco, con la llave en un USB que solo conectas para firmar:

```powershell
minisign -G -p llave-publicacion.pub -s E:\resguardo\llave-publicacion.key
```

- Pide una **contraseña larga** (una frase de 5 o 6 palabras). minisign cifra la llave con ella.
- **Copia de seguridad** de `llave-publicacion.key` (otro USB guardado en otro sitio, o impresa en papel con `type llave-publicacion.key`) y de la contraseña, por separado. Si pierdes las dos, los equipos ya instalados no aceptarán versiones nuevas hasta que los reinstales a mano.
- `llave-publicacion.pub` es **pública**: dos líneas (`untrusted comment: minisign public key <ID>` y la llave en base64, que empieza por `RW`).

Ponla en el repositorio:

1. Sustituye **todo** `packaging/llave-publicacion.pub` por ese archivo (el marcador de posición desaparece).
2. Pon la misma línea `RW…` en `packaging/linux/instalar-agente.sh`, en `LLAVE_PUBLICA="${RESGUARDO_LLAVE_PUBLICA:-PENDIENTE}"` en lugar de `PENDIENTE` (para la instalación a mano con `curl … | sudo sh`).
3. Commit y sube. Desde aquí, `npm run build:agente`, la CI y `construir-paquetes.sh` compilan el agente **con** actualización automática.

**Importante:** los agentes ya instalados (0.7.24 y anteriores) no saben actualizarse solos. La primera versión con la llave se instala **a mano** una vez en cada equipo; a partir de ahí, se actualizan solos.

## 3. Cada versión

1. **Versión y compilación** (como siempre): sube el número en `crates/agente/Cargo.toml` y `crates/servidor/Cargo.toml`, comprobaciones (`AGENTS.md`), `npm run build:agente` y `npm run build:servidor`. Los paquetes de Linux, de la CI (`resguardo-agente-x86_64-linux-musl.tar.gz` y el de `aarch64`).
2. **Una carpeta con lo que se publica**, p. ej. `F:\publicar\0.7.25\`:
   - `Resguardo-Agente_0.7.25_x64-setup.exe`
   - `resguardo-agente-x86_64-linux-musl.tar.gz`
   - `resguardo-agente-aarch64-linux-musl.tar.gz`
3. **Firmar** (conecta el USB de la llave):

   ```powershell
   node scripts/firmar-publicacion.mjs F:\publicar\0.7.25 --llave E:\resguardo\llave-publicacion.key --tambien-paquetes --notas "Arreglos de las copias externas."
   ```

   Deja `manifiesto-agente.json` y `manifiesto-agente.json.minisig` (y un `.minisig` por cada `.tar.gz`). minisign pide la contraseña (una vez por archivo). El script comprueba la firma con minisign y con las mismas reglas que el agente; si dice que no vale, no publiques. Desconecta el USB.
   - `--minimo-desde 0.7.20`: los equipos más antiguos no se actualizan solos a esta (necesitan una intermedia).
4. **GitHub** (publicación `v0.7.25`): sube los archivos, el manifiesto y las firmas:

   ```powershell
   gh release upload v0.7.25 (Get-ChildItem F:\publicar\0.7.25 -File).FullName --repo evercarog/resguardo
   ```

   Los equipos que pueden salir a Internet la verán en GitHub (`releases/latest/download/manifiesto-agente.json`) si sus consolas no la tienen.
5. **Cada Resguardo Server** (para los equipos de su red, aunque no tengan Internet):
   - en la consola: **Servidor → Actualizaciones de los agentes → Subir una versión** (el manifiesto, su firma y los archivos), o
   - en la máquina del servidor: `sudo resguardo-server poner-publicacion /ruta/a/0.7.25` (en Windows, como administrador).

   El servidor comprueba la firma y cada SHA-256 antes de aceptarla.
6. **Despliegue por anillos** (en cada cliente, «Versiones»): los equipos en **prueba** se actualizan enseguida (dentro de su ventana); los de **general**, a los días que diga la política (2 por defecto). Si alguno vuelve atrás, la versión queda **retenida** para el resto del cliente y llega el aviso «actualizacion_fallida».
   - Para parar todo: «En pausa».
   - Para no esperar: «Actualizar ahora» (todo el cliente o un equipo).

## 4. Cambiar de llave (rotación)

1. Crea la nueva (paso 2) con otro nombre de archivo.
2. Añade su `.pub` **debajo** de la actual en `packaging/llave-publicacion.pub` (las dos líneas de cada una).
3. Publica una versión **firmada con la antigua**. Los equipos la instalan y desde entonces aceptan las dos.
4. Las siguientes, fírmalas con la nueva (`--llave …nueva.key`). Cuando todos los equipos estén al día, quita la antigua del archivo.

## 5. Si la llave se pierde o la roban

- **Robada:** cuanto antes, con **otra** llave fijada (paso 4) publica una versión con `--revocar <ID de la robada>` (el ID es el de `untrusted comment: minisign public key <ID>`). Los equipos que la instalen dejan de aceptar para siempre lo firmado con la robada. Mientras tanto, en las consolas, **«En pausa»** en todos los clientes y comprueba qué versión tiene cada equipo. Si la robada era la única fijada, los equipos solo se arreglan reinstalando a mano una versión con la llave nueva.
- **Perdida (sin robo):** si hay otra fijada, como la rotación; si no, la siguiente versión se instala a mano en cada equipo.

## 6. Sin llave (por ahora)

Mientras `packaging/llave-publicacion.pub` sea el marcador de posición, `npm run build:agente` se niega a compilar. Para compilar igualmente un agente **sin** actualización automática:

```powershell
$env:RESGUARDO_SIN_ACTUALIZACIONES = "1"; npm run build:agente
```

En Linux, lo mismo al compilar y al empaquetar (`RESGUARDO_SIN_ACTUALIZACIONES=1 sh packaging/linux/construir-paquetes.sh …`). La consola lo enseña en esos equipos como «Sin actualización automática».
