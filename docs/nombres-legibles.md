# Nombres legibles en el almacén y renombrar repositorios

Diseño del punto **B1** del plan de la 0.8.0 (`plan-mejoras.md`, **14e**). Escrito antes del código, como pide el plan para lo delicado. Vocabulario: [destinos.md](destinos.md); el almacén: [compartir.md](compartir.md) («Fase 4»); el espejo: [espejo.md](espejo.md).

## El problema

En el almacén cada equipo tiene un usuario del rest-server, y ese usuario es también su carpeta (`--private-repos`: el usuario es el primer componente de la ruta). Hasta ahora la consola lo pedía con el **id del equipo** (`guarda_copias { anadir: "<uuid>" }`), así que la carpeta era el UUID. La carpeta de cada repositorio es su **id** en el equipo, y la consola lo inventaba a partir del nombre del **almacén** (`almacen-01-3f2a`), no del repositorio. En el disco del almacén y en los espejos a la nube se ve:

```
mi-bucket/Resguardo/0b5c1f8e-1d2a-4c3b-9e8f-7a6b5c4d3e2f/almacen-01-3f2a
```

Nadie sabe qué es eso sin la consola. Lo que se quiere:

```
mi-bucket/Resguardo/pc-01-3f2a/contabilidad
```

Y además, cambiar el nombre que se ve de un repositorio desde cualquier consola.

## 1. Nombres legibles para lo nuevo

### Cómo se forma un nombre de carpeta

Una sola regla, igual en el agente (`crates/agente/src/nombres.rs`) y en la consola (`consola/src/lib/nombres.ts`), con vectores compartidos (`crates/protocolo/vectors/nombres-legibles.json`):

1. Minúsculas; las letras con tilde o diéresis pierden la marca (`á`→`a`, `ñ`→`n`, `ç`→`c`…).
2. Todo lo que no sea letra `a–z` o cifra pasa a guion; los guiones seguidos, a uno; sin guiones al principio ni al final.
3. Se corta a su largo máximo (sin dejar un guion al final).
4. Si queda vacío: un nombre por defecto (`equipo`, `repositorio`).

Un nombre así solo tiene `a–z`, `0–9` y `-`: nunca `.`, `..`, `/`, `\`, `:`, espacios ni caracteres de control. Es válido para el rest-server, para `.htpasswd` (sin `:`), para una carpeta de Windows y de Linux, y para el espejo (`<usuario>/<repo>`).

### La carpeta (usuario) del equipo

`guarda_copias { anadir: "<id del equipo>", nombre: "<nombre del equipo>" }` (nuevo: `nombre`, opcional). El almacén crea el usuario `<nombre normalizado, hasta 35>-<4 cifras hexadecimales al azar>`, p. ej. `pc-01-3f2a` (como mucho 40). El sufijo evita que dos equipos con el mismo nombre (o uno que se renombró) choquen, y que el nombre adivine otra cosa. El almacén comprueba que el usuario sea **único en todo el almacén** (todas las zonas), que no haya ya una carpeta con ese nombre en ninguna zona (una que quedó de un equipo quitado: el nuevo no debe heredar lo que haya) y que no sea el nombre antiguo de una carpeta movida (ver más abajo); si choca, otro sufijo.

`para` (de cada usuario, ya existía) guarda el id del equipo: así se sigue sabiendo si un equipo ya tiene usuario en una zona (`es_para`), aunque el usuario ya no sea su id.

Sin `nombre` (consola anterior), como siempre: el usuario es el id.

### La carpeta de cada repositorio

Es el **id** del repositorio en el equipo (`crear_repositorio { id }`), así que no hace falta nada nuevo en el agente: la consola, al crear un repositorio **en un almacén**, le da como id el nombre del repositorio normalizado (hasta 40), único entre los repositorios de ese equipo y entre las carpetas que el almacén ya tiene para ese usuario (`guarda_copias.repositorios`): `contabilidad`, `contabilidad-2`… En los demás destinos (una nube, una carpeta) se sigue con el sufijo al azar, porque varios equipos pueden compartir el bucket o la carpeta.

### Lo que ya existe

Nada se mueve solo. Los usuarios con UUID y las carpetas con el nombre del almacén siguen funcionando igual. Para cambiarlos está «Ordenar nombres de carpeta» (§3).

## 2. Renombrar un repositorio

El nombre que se ve vive en el **equipo dueño** (`RepoV2.nombre`) y llega a todas sus consolas en el resumen. Orden nueva **`nombre_repositorio { repo, nombre }`**:

- **Inofensiva** (no revela nada ni reduce la protección, como `nombre_equipo`), **solo administradores** (como el nombre del equipo).
- El agente valida el nombre como al crearlo (de 1 a 80 caracteres, sin caracteres de control), solo para repositorios que gestiona (también los de solo lectura importados: es su nombre en este equipo).
- No cambia la carpeta ni el id: el kit y la restauración siguen igual.
- Queda en el historial común («Cambiar el nombre de un repositorio a «Ventas»») y se sube la configuración a todas las consolas al momento (`CAMBIAN_CONFIG`).
- `admite: "nombres_legibles"`. A un agente anterior la consola no le ofrece «Cambiar el nombre».

## 3. Ordenar nombres de carpeta (mover lo que ya existe)

Para pasar `0b5c…/almacen-01-3f2a` a `pc-01-3f2a/contabilidad`. Es lo delicado: el almacén, el equipo dueño y los espejos tienen que quedar de acuerdo. **Nunca a mano**: si alguien renombra la carpeta en el disco del almacén, el equipo deja de poder copiar y el espejo cree que el repositorio desapareció.

### Quién hace qué

1. **El almacén** (`guarda_copias { mover: { usuario, legible?, repos?: [{ de, nombre }] } }`, clave de administración):
   - `usuario`: el usuario actual del equipo en el almacén.
   - `legible`: si viene, el usuario pasa a `<legible normalizado>-<sufijo>` (lo elige el almacén, único como en §1).
   - `repos`: carpetas de repositorios de ese usuario que pasan a `<nombre normalizado>` (único dentro del usuario: `-2`, `-3`… si hace falta).
   - Comprueba **antes de tocar nada**: que el usuario es de este almacén; que las carpetas existen, son repositorios (`config`) y no hay enlaces en el camino; que el destino no existe; que no hay una copia, una poda ni una verificación en marcha en esos repositorios (un archivo en `locks/` de menos de 15 min) ni un espejo en marcha en el almacén.
   - Lo hace en este orden, guardando cómo deshacer cada paso: renombra cada carpeta de repositorio dentro de la del usuario; renombra la carpeta del usuario; cambia el usuario en su `.htpasswd` (la misma contraseña: solo cambia el nombre delante del hash); pone al día la retención del almacén (`usuario/repo` de cada regla), la selección de los espejos (`que.repos`, `que.equipos`, `vistos`, también la vista de antes `destinos[].repos`) y apunta el cambio en `renombres` (lo que usa el espejo, abajo); guarda la configuración. **Si algo falla, deshace lo hecho** en orden inverso y dice qué no pudo deshacer (no debería pasar: son renombres en el mismo disco).
   - Todo en el mismo disco: `rename` es atómico (no se copia nada, no hay un momento con medio repositorio).
   - Si cambió el usuario, espera a que el rest-server lea el `.htpasswd` nuevo (lo relee al llegar una petición, como mucho cada 30 s; en Linux se le avisa con `SIGHUP`), para que el paso 2 ya entre con el usuario nuevo.
   - Contesta con lo que hizo: `detalle: { mover: { usuario?: { de, a }, repos: [{ de, a }] } }` (solo nombres de carpeta, sin secretos).
2. **El equipo dueño** (`cambiar_destino { destino, donde?, usuario?, carpetas?: { "<id del repositorio>": "<carpeta>" } }`, clave de administración; `carpetas` es nuevo): pone la dirección nueva (`https://ip:puerto/<usuario nuevo>/`), el usuario nuevo (la contraseña no cambia) y la carpeta nueva de cada repositorio (`ubicacion_origen`; el id no cambia). Antes de guardar **abre cada repositorio** con los datos nuevos; si alguno no abre, no guarda nada. Con el usuario cambiado, si el rest-server aún no lo acepta, lo reintenta durante unos 45 s.
   - Un equipo con el agente anterior entiende `donde` y `usuario`, pero no `carpetas`: la consola solo le ofrece mover la carpeta del equipo (el usuario), no la de sus repositorios.
   - Lo mismo, a los equipos que tienen ese repositorio **importado** (solo lectura): la misma orden, con su destino.
3. **Los espejos**: los del almacén se ajustan en el paso 1 (la selección `pc-01-3f2a/contabilidad`). En el **destino del espejo** no se mueve nada:
   - La carpeta nueva se copia entera en la próxima vuelta (en una nube, se **vuelve a subir**: la consola lo dice con el tamaño antes de confirmar).
   - La carpeta vieja **no se borra** por moverla: queda para la retención del espejo. Con «nunca borra», se queda para siempre (se puede borrar a mano cuando la nueva esté subida); con retraso de N días o «igual que el origen», se borra a su tiempo, como cualquier cosa que ya no está en el almacén. Mientras tanto se sigue pudiendo restaurar desde ella.
   - **El freno no salta por esto.** Que desaparezca de golpe un repositorio entero (`config`) frena el espejo; aquí desaparece a propósito. El almacén apunta cada cambio en `renombres: [{ de, a, cuando, zona? }]` (los últimos 200) y el motor del espejo no cuenta para el freno lo que falta bajo una ruta `de` (sí lo cuenta para su retención). En un espejo con selección, la ruta vieja sigue entrando en lo que se mira del destino, para que su retención la vea.
4. **La retención del almacén** sigue igual (misma regla, misma clave del almacén, que está dentro del repositorio y se mueve con él); solo cambia su `usuario/repo`.

### Qué ve la persona

En la ficha del almacén, **«Ordenar nombres de carpeta…»** (y, en la página de un repositorio que está en un almacén, «Ordenar su carpeta…»). Por equipo:

- Ahora → después, con las carpetas de verdad: `0b5c…/almacen-01-3f2a` → `pc-01-3f2a-…/contabilidad` (el sufijo lo pone el almacén).
- Qué más cambia: «El espejo «Todo a la nube» volverá a subir la carpeta nueva (unos 70 GB); la vieja se queda en la nube hasta que la quite su retención» (o «…se queda para siempre: este espejo nunca borra»).
- Requisitos, con el botón apagado diciendo qué falta: el equipo dueño conectado; el almacén y el equipo con el agente que lo admite (`nombres_legibles`); la clave de administración.
- Pasos con su estado: «El almacén mueve las carpetas» → «PC-01 usa la ruta nueva» (→ «PC-07, que lo tiene importado, también»). Si el segundo falla o el equipo no contesta: **qué falta**, «Reintentar en PC-01» y «Deshacer en el almacén» (la misma orden al revés).
- Si se cierra el navegador a medias, la consola lo nota sola: el almacén dice sus `renombres` y el equipo sigue con la ruta de antes. En la ficha del equipo y en la del almacén sale «La carpeta de «Contabilidad» cambió en ALMACEN-01, pero PC-01 sigue con la de antes» con **«Poner la ruta nueva»**.
- En el historial común: «Ordenar los nombres de carpeta de pc-01 en el almacén» (almacén) y «Cambiar las credenciales del destino» (equipo).

### Por qué así

- **Dos órdenes y no una.** El almacén no tiene autoridad sobre la configuración del equipo, ni el equipo sobre el disco del almacén. La consola, con la clave de administración, coordina; cada uno comprueba lo suyo. Es lo mismo que «Copiar en …» (dar acceso + crear el repositorio).
- **Primero el almacén.** El equipo no puede comprobar la ruta nueva hasta que existe. Entre los dos pasos, el equipo no puede copiar (unos segundos): por eso se pide que esté conectado y sin copiar.
- **Sin espera.** Mover no borra nada ni reduce la protección: todo queda en el mismo disco, en otro nombre, y en el espejo la carpeta vieja se queda. Se puede deshacer. Pide la clave de administración, como dar acceso.
- **No se mueve en el destino del espejo** (se valoró `rclone moveto`, o `rclone copy` de servidor a servidor, para no volver a subir):
  - en S3 y B2, «mover» es copiar en el servidor y **borrar**: con bloqueo de objetos (Object Lock) no se puede borrar (o deja marcas de borrado y versiones que cuestan), justo donde más importa no tocar nada;
  - en Dropbox y Drive sí hay un mover de verdad, pero el espejo nunca ha borrado ni movido nada fuera de su retención y su freno, y es lo que lo hace fiable frente a un almacén comprometido: un almacén atacado que pudiera «mover» en la nube podría vaciar lo que el espejo protege;
  - en una carpeta del mismo disco sería gratis, pero entonces el espejo se comportaría distinto según el destino.
  Se deja para más adelante como opción explícita («Mover también en <destino>», solo nubes sin bloqueo y carpetas, con la clave), si volver a subir resulta un problema real.

## Seguridad

- Los nombres vienen de fuera (consola): el agente normaliza o valida todo (`a–z`, `0–9`, `-`; largos; nunca `..`, `/`, `\`, `:` ni control) y nunca usa un nombre sin comprobarlo. Nada de `panic` con datos externos.
- Un equipo nunca llega a la carpeta de otro: el usuario nuevo no puede ser uno que existe, ni una carpeta que ya está en el disco, ni el nombre antiguo de otra movida (si no, un equipo nuevo heredaría en el espejo la carpeta vieja de otro). `--private-repos` sigue igual.
- Sin enlaces en el camino (como la retención del almacén): ni la carpeta del usuario ni la del repositorio pueden ser un enlace o una unión.
- Nada se borra: solo se renombra en el mismo disco. Lo que no se puede renombrar (otro disco, un enlace), no se hace.
- El resumen solo lleva nombres de carpeta (como ya llevaba `repositorios`).

## Contrato (compatible hacia atrás)

En `api-servidor.md` → «Cambios», como «v1.6x (pendiente de numerar al unir)»:

- `admite: "nombres_legibles"`.
- `guarda_copias { anadir, nombre? }`; `guarda_copias { mover: { usuario, legible?, repos?: [{ de, nombre }] } }` → `detalle.mover`.
- `guarda_copias.renombres: [{ de, a, cuando, zona? }]` en el resumen.
- `cambiar_destino { …, carpetas?: { "<repo>": "<carpeta>" } }`.
- Orden nueva `nombre_repositorio { repo, nombre }` (inofensiva, solo administradores).
- `retencion_almacen` y `aplicar_retencion_almacen` con `nombre?` («Contabilidad, de PC-01»), solo para la descripción en el historial y en las órdenes en espera (sin él, `usuario/repo` como antes).

## Lo que no se hace (todavía)

- Mover en el destino del espejo (arriba).
- Ordenar las carpetas de las **copias derivadas** que van a un almacén (`ruta` de la derivada): siguen con su nombre. Se mueven con la del usuario si cambia la del equipo.
- Ordenar todo el almacén de una vez: se hace equipo a equipo (cada uno necesita su propia orden y estar conectado).
