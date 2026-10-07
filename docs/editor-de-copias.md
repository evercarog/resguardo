# El editor de copias: un solo sitio para añadir, cambiar y ordenar

Rediseño pedido por el responsable el 2026-10-06 (rama `ia/editor-de-copias`), después de probar las copias en cadena ([copias-en-cadena.md](copias-en-cadena.md)). Lo dudoso está en [registro-ia.md](registro-ia.md).

## Qué no se entendía

1. **Ordenar**: las flechas de subir y bajar casi no se veían; nadie sabía que las copias se ordenan.
2. **Un estado imposible**: una copia movida al primer puesto seguía diciendo «después de "Nueva copia"». La primera no puede ir después de otra.
3. **Opciones escondidas**: el espejo de una copia, el repositorio nuevo a partir de otra y «después de la anterior» estaban detrás de «Añadir una copia», un diálogo que acababa en el editor igualmente.
4. **Las copias, abajo**: en la ficha del equipo, las copias quedaban debajo del mapa. Es lo que más importa.
5. **El menú «Más…» del repositorio** guardaba la retención, la prueba de restauración, la verificación, la copia externa y las derivadas. Nadie las buscaba allí.

## Referencias

Solo documentación y artículos públicos, sin entrar en ninguna cuenta.

| Producto | Qué hace con el orden y los pasos encadenados | Qué tomamos |
|---|---|---|
| **Zapier** (editor visual) | Lista vertical de pasos unidos por una línea; un **«+» en cada unión** para insertar ahí; los pasos se arrastran. El primero es el disparador y es de otro tipo. ([ayuda](https://zapier.com/blog/maximize-productivity-with-multi-step-zaps/), [editor visual](https://www.xray.tech/post/zapier-visual-editor), [reordenar](https://community.zapier.com/how-do-i-3/how-do-i-rearrange-actions-or-steps-in-a-zap-12578)) | Línea que une los pasos encadenados; «+ Añadir paso» en cada copia; el primero solo empieza con horario. |
| **n8n / Make** | Lienzo con nodos y conectores; el disparador siempre a la izquierda. | La conexión se **dibuja**, no se explica con texto. Un lienzo libre es demasiado para una lista corta: no lo copiamos. |
| **GitHub Actions** (`needs`) | El gráfico de la ejecución dibuja una línea entre cada trabajo y los que necesita. ([docs](https://docs.github.com/en/enterprise-server@3.17/actions/how-tos/monitor-workflows/use-the-visualization-graph)) | «Después de» es una flecha entre dos tarjetas. |
| **Veeam** (copia de una copia de seguridad) | El modo «Immediate copy» copia en cuanto termina el trabajo de origen; el trabajo de copia **elige qué trabajos copia**. ([guía](https://helpcenter.veeam.com/docs/vbr/qsg/backup_copy.html), [buenas prácticas](https://bp.veeam.com/vbr/4_Operations/O_Veeam_Jobs/O_backup_copy_jobs/backup_copy_job.html)) | Lo que cuelga de un repositorio (espejo, derivada) se añade desde ese repositorio, no desde una lista general. |
| **Synology Hyper Backup** | Tareas con horario; si coinciden, van en cola. No hay «después de». ([ayuda](https://www.synology.com/knowledgebase/DSM/help/HyperBackup/data_backup_create)) | Confirmamos que «después de la anterior» es una ventaja: hay que hacerlo **obvio**. |
| **Arq 7** | «Planes de copia» independientes, cada uno con su destino. ([docs](https://www.arqbackup.com/docs/arqbackup/pages/multiple_destinations.html)) | Cada copia es una tarjeta con su destino a la vista. |
| **Notion / Linear** | Asa de seis puntos a la izquierda del bloque; al arrastrar, una raya fina dice dónde cae. ([Notion](https://www.notion.com/help/writing-and-editing-basics)) | Asa ⠿ siempre visible (no solo al pasar el ratón: en el móvil no hay ratón) y la raya de destino. |
| **Apple Atajos** | Mantener pulsado y arrastrar una acción. ([ayuda](https://support.apple.com/guide/shortcuts/reorder-shortcuts-apd052072d68/ios)) | Arrastrar también con el dedo (eventos de puntero, no el arrastre de HTML, que en el móvil no funciona). |
| **Listas reordenables accesibles** ([Darin Senneff](https://www.darins.page/articles/designing-a-reorderable-list-component), [Smashing](https://www.smashingmagazine.com/2018/01/dragon-drop-accessible-list-reordering/), [Salesforce](https://salesforce-ux.github.io/dnd-a11y-patterns/)) | Arrastrar es un extra: subir/bajar con teclado y botones; asa de alto contraste; anunciar «X, posición 2 de 3». | El asa es un botón: flechas ↑/↓ mueven, y se anuncia en una región viva. «Subir» y «Bajar» también en el menú de la tarjeta. |
| **NetBird / Tailscale** (consolas) | Tablas tranquilas con un menú «…» por fila para editar y borrar; los ajustes de cada cosa en su propia ficha. ([Tailscale](https://tailscale.com/docs/features/visual-editor)) | «Más…» solo para lo raro (mover, quitar). Lo frecuente, en botones a la vista. |

**Patrones comunes:** lista vertical numerada; conexión dibujada; «+» donde se quiere añadir; el primer paso es especial (el disparador); asa visible; menú «…» solo para lo raro; pocas palabras.

## Tres opciones

### A. Lista numerada con asa (como ahora, pulida)

```
 1 ⠿ Documentos            Con horario · Lun–Vie 13:00   [Activa] [⋯]
 2 ⠿ Fotos                 Después de «Documentos»       [Activa] [⋯]
```

Fácil, pero «después de» sigue siendo texto: hay que leer para entender la cadena.

### B. Flujo vertical con conectores (elegida)

```
 ┌─ 1 ⠿ Documentos ───────────────────────────── ● Activa  ⋯ ┐
 │  🕑 Con horario · De lunes a viernes a las 13:00            │
 │  📁 3 carpetas → 🗄 Almacén · Disco D                        │
 │     ↳ espejo Disco E · ↳ copia a B2                         │
 │  Retención · Verificación · Prueba     [+ Añadir paso]      │
 └─────────────────────────────────────────────────────────────┘
      │  después
      ▼
 ┌─ 2 ⠿ Fotos ───────────────────────────────── ● Activa  ⋯ ┐
 │  🔗 Después de la anterior                                  │
 │  …                                                          │
 └─────────────────────────────────────────────────────────────┘

 (sin conector: empieza con su horario)

 ┌─ 3 ⠿ Contabilidad ─────────────────────────────────────────┐
 └─────────────────────────────────────────────────────────────┘
 [+ Añadir una copia]
```

- Cada copia es una tarjeta numerada con su asa ⠿. Entre dos tarjetas, **la flecha «después» solo se dibuja si la de abajo va después de la de arriba**. Sin flecha, empieza con su horario.
- **Cuándo empieza** es un control de dos botones: «Con horario» y «Después de la anterior». En la primera, el segundo **está desactivado** («La primera empieza con su horario»).
- Al ordenar, «después de la anterior» **sigue a la de encima** (se vuelve a enlazar con la nueva anterior). Si una copia con «después» llega al primer puesto, pasa a «con horario» (el de antes o uno por defecto) y se avisa en una línea: «"Fotos" es ahora la primera: empieza con su horario».
- Cada tarjeta se pliega: cerrada, enseña el resumen y su cadena; abierta, el formulario de siempre.

### C. Lienzo libre (como n8n)

Nodos y flechas en dos dimensiones. Muy potente para flujos con ramas, pero aquí son 1–6 copias por equipo; en el móvil no funciona bien y pide mucho más código. Descartada.

**Por qué B:** dibuja la cadena (no hay que leer para entenderla), tiene el «+» donde se piensa en añadir (Zapier), el primero es especial como el disparador, y sigue siendo una lista: accesible con teclado y en el móvil.

## La tarjeta de cada copia

De arriba abajo, con iconos y etiquetas cortas (los detalles, en *tooltips*):

1. **Cabecera:** número, asa ⠿, nombre (editable), chip del estado (Activa / Desactivada / Nueva / Cambiada), plegar, y «⋯» (Subir, Bajar, Guardar como plantilla, Rellenar con…, Quitar).
2. **Cuándo:** «Con horario» | «Después de la anterior». Con «después», el interruptor «Además, con su horario».
3. **Su cadena:** `📁 carpetas → 🗄 repositorio en su destino`, y debajo lo que cuelga del repositorio: espejos (del almacén), copia externa y copias derivadas, cada una con su icono.
4. **Repositorio** (plegado dentro de la tarjeta): Retención (abre su diálogo), Verificación automática y Prueba de restauración automática (en línea: son de la configuración y se envían con el resto). Si otra copia usa el mismo repositorio, se dice: «También lo usa "X"».
5. **«+ Añadir paso»**, un menú con tres opciones, siempre a la vista (las que no se pueden, desactivadas con el motivo):
   - **Copia nueva (carpetas)**: una tarjeta nueva justo debajo, «después de la anterior».
   - **Espejo de esta copia**: el mismo repositorio en otro destino; lo hace el almacén (`PasoEspejo`).
   - **Repositorio nuevo a partir de esta copia**: otro repositorio con su contraseña, que va por su cuenta desde ahí (`CopiaDerivada`).

El espejo, la derivada y la retención son **órdenes aparte** (a otro equipo, o con la contraseña del repositorio): se envían en su diálogo, al momento, sin perder lo que se está editando. Las copias, la verificación y la prueba van juntas con «Enviar al equipo».

## Arrastrar y teclado

- **Ratón y dedo:** se arrastra desde el asa (eventos de puntero con captura; `touch-action: none` solo en el asa, así la página sigue desplazándose con el dedo en el resto). Una raya de acento dice dónde caerá.
- **Teclado:** el asa es un botón. Flecha arriba/abajo mueve la copia y el foco la sigue. Inicio/Fin, a la primera o la última.
- **Lector de pantalla:** el asa dice «Mover "Fotos", posición 2 de 3» y cada movimiento se anuncia en una región viva.
- Respeta `prefers-reduced-motion`.

## Dónde vive cada acción

Regla: **cada acción se alcanza desde donde se está mirando esa cosa.**

| Acción | Ficha del equipo | Página de la copia | Página del repositorio | Editor de copias |
|---|---|---|---|---|
| Añadir una copia | «Añadir o cambiar copias» (botón principal de la sección) | — | — | «+ Añadir una copia» y «+ Añadir paso → Copia nueva» |
| Cambiar una copia (carpetas, horario, cuándo empieza) | «Añadir o cambiar copias» | «Cambiar» (abre el editor con esa copia abierta) | — | la tarjeta |
| Ordenar | — | — | — | asa ⠿, flechas, «⋯ → Subir/Bajar» |
| Espejo / Repositorio nuevo a partir de esta | botones del repositorio | «+ Añadir paso» | «+ Añadir paso» | «+ Añadir paso» |
| Retención | botón «Retención» del repositorio | «Retención» | «Retención» | en la tarjeta |
| Verificación automática | «Verificación» del repositorio | «Verificación» | «Verificación» | en la tarjeta |
| Prueba de restauración (automática / ahora) | «Prueba» del repositorio | «Prueba» | «Probar ahora» y «Automática…» | en la tarjeta |
| Copia externa / derivadas (cambiar, subir ahora, quitar) | en la tarjeta del repositorio, como hasta ahora | lista de su cadena | lista de su cadena | — |
| Traer historial | — | — | botón | — |
| Mover, dejar de copiar, quitar | «Más…» | — | «Más…» | «⋯ → Quitar» (la copia) |

**Ficha del equipo:** la sección «Copias» sube justo debajo de las cuatro cifras, antes del mapa, con el botón principal **«Añadir o cambiar copias»**. «Añadir una copia» (el diálogo previo) desaparece: todo empieza en el editor.

## Lo que no cambia

- El contrato con los agentes: la configuración sigue siendo `config.copias[]` con `tras` (v1.55). Solo cambia cómo la consola la edita. La comprobación de cadenas (`errorCadenas`) sigue igual.
- La clave de administración: el editor se abre con ella y «Enviar al equipo» firma la orden `config`, como siempre.
- La tira 3·2·1·1·0 y la página de los destinos (las lleva otra rama).

## Elegir el destino de un paso

Lo pidió el responsable con un caso real: una Dropbox conectada en el almacén (para su espejo) no aparecía al añadir un paso desde el equipo que copia.

- «Espejo de esta copia» y «Repositorio nuevo a partir de esta» enseñan **todos los destinos del cliente** (zonas, destinos de los equipos, catálogo y nubes conectadas en cualquier equipo), los que sirven primero. Los que no, desactivados y con el porqué en pocas palabras:
  - **Espejo:** lo hace el almacén donde está el repositorio, así que sirven sus otras zonas y sus nubes. Una nube conectada en otro equipo: «Hace falta también en …» con «Conectar en …».
  - **Repositorio nuevo a partir de esta:** lo hace el equipo dueño. Una Dropbox conectada solo en el almacén: «Conectar en …» (el flujo de siempre, `conectar_nube`, en el equipo dueño; 4a). Con un agente sin `nube_equipo`: «Actualiza el agente de …». Un B2 o S3 del catálogo que el equipo aún no tiene: se elige y pide solo sus credenciales.
  - **Copia nueva (carpetas):** directo a una nube (4a, rama `ia/repos-en-la-nube`) si su agente anuncia `repo_en_nube` y la nube está conectada en el equipo; si está en otro (el almacén), «Conectar Dropbox también en …». Con un agente anterior, las nubes salen desactivadas en el repositorio de la copia con el camino que sí: actualizar el agente, o copiar al almacén y después «Repositorio nuevo a partir de esta».
- **«+ Repositorio nuevo…»** en el selector del repositorio de cada copia (y en el aviso de un equipo sin repositorios): el mismo diálogo que «Nuevo repositorio» (`lib/repoNuevo.ts`), con el equipo fijo y la autorización que ya se calculó al abrir el editor (no se vuelve a pedir la clave, salvo para un almacén o al conectar una nube, que son órdenes a otro equipo). El repositorio queda elegido en la copia; el equipo lo crea antes de aplicar la configuración (las órdenes van en orden).
- La lógica está en `lib/cadenas.ts` (`destinosParaPasos`, `usosPosibles`, `detalleDestino`, con vectores) para que la página de cada destino pueda ofrecer «Usar en una copia» con lo mismo.
