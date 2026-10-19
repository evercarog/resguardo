# Revisión de detalles menores (2026-10-19)

Cuatro revisiones de solo lectura del código (sin arrancar la consola ni probar en un navegador), hechas para el bloque G de [plan-0.8.1.md](plan-0.8.1.md): textos y voz de la consola; estados de las pantallas (vacíos, carga, errores, confirmaciones); accesibilidad y pantallas estrechas; y lo que ven las personas fuera de la consola (agente, bandeja, ventana, instalador, avisos y correos del servidor). Las rutas son relativas a `consola/src` salvo que digan otra cosa. Al hacer cada punto, comprobar que sigue igual en el código: esto es una foto del 2026-10-19.

## G1. Fallos de verdad (primero)

- **Variables de CSS que no existen:** `--text`, `--text-muted`, `--danger`, `--danger-text`, `--warn-text` en `espejos/ListaEspejos.svelte` (312, 315, 324, 338), `espejos/EditorEspejo.svelte` (644, 735) y `VersionNueva.svelte` (28): los fallos de los espejos no salen en rojo. `--font-mono` (el token es `--mono`) en 8 componentes; `--fs-md` (no existe) en 11 sitios; `--sp-16` que `docs/diseno.md` lista y `ui/src/estilos.css` no define. Una comprobación estática en `test:vectores` que avise de variables `var(--…)` sin definir.
- **Escape cierra cualquier diálogo** aunque tenga `dismissible={false}` (`ui/src/componentes/Modal.svelte:57`): asistentes largos pierden lo escrito (Nuevo repositorio, Editor de espejo, Mover repositorio, Restaurar en otro, Conectar destino, Elegir carpetas), diálogos que bloquean el cierre mientras envían se cierran igual (Cambiar clave, OrdenDialog) y se pierden códigos de un solo uso (ajustes del cliente, invitación de propietario en `servidor/clientes`). Que Escape respete `dismissible` o pregunte «¿Descartar lo escrito?».
- **Cargas que no terminan nunca** si falla la petición: pestaña «Informes» del equipo (`equipos/[e]/+page.svelte:1546`), Avisos, Órdenes, la portada y Clientes (`cargarClientes` sin `catch`), las secciones de «Todos los clientes» (solo `/todos` enseña el error), Pruebas funcionales y Versiones (cualquier fallo se lee como «actualiza Resguardo Server»). Un `PantallaError` común con «Reintentar».
- **Un informe para el cliente con datos en blanco sin avisar:** `informes/+page.svelte` (72, 87) traga los fallos con `.catch(() => [])` e imprime «0 copias». Marcar «faltan datos de X».
- **Doble clic, doble orden:** botones que no se apagan mientras envían: crear invitación (dos enlaces válidos 7 días), quitar persona, quitar canal, borrar plantilla, anular códigos de alta, renombrar el cliente (acepta vacío). `BotonCargando` en todos.
- **Restaurar, paso «¿Qué archivos?»:** si falla listar o buscar, el árbol queda en blanco sin aviso; la búsqueda no dice «Buscando…».
- **Restauraciones:** con error salen a la vez el error y «Aún no hay restauraciones».
- **Errores inesperados en inglés:** no hay `hooks.client.ts` con `handleError`, y `+error.svelte` enseña «Internal Error».
- **Enlace roto:** `equipos/[e]/espejos/+page.svelte:109` lleva a `#guarda`, que no existe.
- **Prueba de restauración contada como verificación** en los avisos de Windows (`crates/agente/src/escritorio.rs:191`).

## G2. Confirmar y no perder trabajo

- Avisar al salir del editor de copias y del asistente de Restaurar con cambios sin enviar (`beforeNavigate`).
- Confirmación en: «Borrar la copia guardada» de la consola (`servidor/+page.svelte:304`, que además dice «borrada» aunque no hubiera ninguna), borrar un adjunto de una prueba funcional (lo borra al momento aunque luego se cancele), cancelar una orden en `ListaOrdenes` (otras listas sí preguntan).
- `dismissible={false}` en el formulario de prueba funcional y en la invitación de propietario.
- Copiar al portapapeles: si falla (consola por http), decirlo y dejar el texto seleccionado.
- Botones apagados que no dicen qué falta: Restaurar en otro equipo, Adoptar, Mover cliente, Mover repositorio, Traer historial, Servidores de respaldo, Añadir equipo, Cambiar clave, Mover a consola, Conectar consola: pasarlos a `BotonFalta`. Quitar el botón apagado «Actualizar el agente · próximamente».
- Estados vacíos con su botón: copias de un equipo, «Todos los clientes» → Copias y Equipos, Estado (y, con un filtro de etiqueta sin resultados, «Ningún equipo con esa etiqueta · Quitar el filtro» en vez de «Añade el primer equipo»).

## G3. Un solo vocabulario en la consola

- «copia externa» visible aún en Restaurar, `restauraciones.ts`, `destinos.ts` y la página del repositorio → «copia derivada».
- Jerga en textos: «(forget, sin prune)», «(forget --prune)» en `copiaExterna.ts`; «Bucket» y «endpoint» sin traducir; «id» → «identificador».
- `inmutable` de un destino enseñado como «solo añadir» en la ficha del equipo y de la copia (contra 14z).
- **Una sola tabla de tipos de destino** (`TEXTO_TIPO`): hoy «Disco», «Disco del equipo», «Disco o carpeta del equipo» y «Carpeta de este equipo» para lo mismo, y le falta `otro`. Un solo nombre para el servidor REST de fuera («Servidor de copias» / «de fuera» / «externo») y para las credenciales de B2 y S3 (cuatro nombres).
- Retención: «quitar», no «eliminar».
- Botones: «Ver los informes», «Ver el equipo» (con artículo y minúscula); «Reintentar» o «Volver a intentarlo», uno; «Nuevo repositorio» o «Repositorio nuevo», «Nueva copia» o «Copia nueva», uno; «Añadir un espejo»; «Seguir» para avanzar y «Listo» para terminar (hoy «Listo», «Hecho», «Entendido», «Cerrar», «Seguir», «Siguiente»).
- «Órdenes destructivas», «lo destructivo espera» (vetado en `diseno.md`) → «órdenes con espera».
- Unidades: velocidades en «MB/s» calculadas en KiB y «KiB/s» en etiquetas; espacio fino antes de la unidad; fechas relativas con `<Tiempo>` también al restaurar desde un espejo.
- Erratas y frases: «Ir a entrar» → «Entrar»; «espera que compruebes» → «espera a que»; partir tres textos de más de 300 caracteres (`cadenas.ts:361`, servidor, «Comprobar la contraseña»).
- Mayúsculas a mitad de frase: «Fuera del sitio o en la Nube».
- Desplegables: «Other», «Minio», «nextcloud», «owncloud» → «Otro», «MinIO», «Nextcloud», «ownCloud».

## G4. Fuera de la consola: agente, bandeja, ventana, instalador, avisos

- **Avisos y correos:** «Pendiente en «X»: aplicar_retencion_almacen» (nombre interno, sin fecha) → la descripción y cuándo se aplica; «Orden destructiva pendiente» → «Orden con espera»; «Cambio inusual» como cajón de sastre → un título por caso; «Falló el espejo de «equipo»» sin decir qué espejo; retenciones con la carpeta en vez del nombre del repositorio.
- **Errores de restic pasados tal cual** (`crates/motor/src/restic.rs:448`, «Falló: …» en inglés o en JSON) a la bandeja, Windows, la consola y el correo (con 15oo).
- **«restic» en el kit** (`cli.rs:444`, ventana `Kit.svelte`): decidir si el kit es la excepción («el programa de copias (restic)») y decirlo una vez.
- **«Código» y «número» de comprobación** mezclados (agente, registro, ventana) → «número de comprobación».
- **«Emparejar» y «vincular»** en el instalador → «Vincular con la consola».
- **«Servidor de copias» y «rest-server»** para el almacén en la ayuda, la ventana y el instalador → «almacén».
- **«copia externa»** en `cli.rs`, `escritorio.rs` y la ventana → «copia derivada».
- **Instalador:** dice «No tiene ventana» y «No permite cambiar nada» (las dos cosas ya no son ciertas); al ofrecer borrar los datos no avisa de que sin el kit no se podrán abrir las copias; menciona la app antigua; pide escribir `resguardo-agente` sin ruta (con 15o); lanza la bandeja como administrador (con 15o).
- **Ayuda de `resguardo-agente`:** separar «Consolas y órdenes» del modo local; documentar `--nombre-destino`; quitar el ejemplo con nombre de empresa (como 14y); sin mayúsculas de énfasis («ERROR», «SIN RESPONDER», «MISMA»); el horario del espejo dicho igual que en la consola.
- **Errores en inglés o técnicos:** ventana (`ventana.rs`: rclone, WebView2, «no se abre como administrador» sin qué hacer), `cli.rs` (serde «no es JSON», «(--cacert)»), «No se encontró el ejecutable de restic… PATH» en la línea de órdenes, API («Logo no válido (base64).», «No existe.» sin decir qué).
- **Bandeja y ventana:** «Resguardo Server», «servidor» y «consola» para lo mismo → «consola»; «Copiando el espejo a» y «Subiendo a» → uno; «sitio» → «destino»; «Algunos archivos no se pudieron leer» solo para copias.

## G5. Accesibilidad

- Chips de estado sin icono (llaves cambiadas, sin responder, sin propietario, la tira 3-2-1): usar `<Chip>`.
- «Recomendado» con el verde de estado → tono neutro o acento.
- Cambios de una versión (añadido, cambiado, borrado) solo por icono y color: texto para lectores de pantalla.
- Tabla de la regla en Informes con cabeceras «3», «2», «1», «1», «0»: su nombre en `sr-only` o `abbr`.
- Motivo de un botón apagado solo en `title` o en un tooltip (Informe ejecutivo, Quitar destino): visible o con `aria-describedby`.
- `title` nativo en 10 sitios (prohibido en `diseno.md`): a `use:tip`.
- Pestañas de «Organizar equipos» sin teclado: flechas o `role="group"` con `aria-pressed`.
- Nombre accesible que tapa la etiqueta visible (filtro «Espejos» en Copias).
- Barra de fuerza de la clave sin `aria-valuetext`; campos de hora con el nombre «a» → «Desde» y «Hasta».
- Zonas con desplazamiento lateral sin `tabindex` ni nombre.

## G6. Pantallas estrechas (móvil)

- Tabla de pruebas funcionales en Informes sin `.desliza`: desborda a 375 px.
- `.pastilla` corta las direcciones largas sin puntos suspensivos; las fichas de restauraciones y pruebas desbordan con rutas largas (`minmax(0,1fr)` y `overflow-wrap`).
- Campos a 14 px: Safari en iPhone amplía la página al enfocarlos (16 px en pantallas estrechas).

## G7. Sistema de diseño (orden interno)

- Títulos de diálogo y cifras grandes con tamaños a mano (17, 18, 20, 22 px, `rem` en un componente) → tokens `--fs-*`.
- Contornos de las muestras de color que desaparecen en oscuro; escala `--calor-*` definida en un componente y usada en otro → a `ui/src/estilos.css`.
- Repetidos que deberían ser comunes: `.sr-only` redefinida, buscador con icono copiado en 5 páginas, bloque de impresión del kit copiado en 6 componentes, barra de fuerza de la clave en 2, `.desliza` y `.desplazable` para lo mismo.
